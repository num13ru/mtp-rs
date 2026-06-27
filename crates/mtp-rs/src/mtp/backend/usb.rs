//! The PTP-over-USB backend: today's [`PtpSession`] logic behind the neutral [`MtpBackend`] trait.
//!
//! All device-quirk handling lives here (the root-listing fast path and Android/Samsung/Fuji
//! fallbacks, the `>4 GB` size resolution, the upload partial-handle contract, SIC class-cancel,
//! and session self-healing). The trait boundary is the only place PTP types and the rich
//! [`crate::PtpError`] convert to the neutral [`crate::mtp`] vocabulary, via the `from_ptp`/`to_ptp`
//! helpers and the `From<PtpError>` error map. nusb, the virtual device, and the mock transport all
//! ride this one backend through their [`Transport`](crate::transport::Transport) impls.

use crate::cancel::{bail_if_cancelled, CancelToken};
use crate::mtp::backend::{
    BackendDownload, BackendListing, ByteRange, DownloadBody, MtpBackend, ProgressFn, UploadStream,
};
use crate::mtp::object::NewObjectInfo;
use crate::mtp::stream::Progress;
use crate::mtp::{
    Capabilities, DeviceEvent, DeviceInfo, Error, ObjectHandle, ObjectInfo, StorageId, StorageInfo,
    UploadError,
};
use crate::ptp::{
    DeviceInfo as PtpDeviceInfo, ObjectHandle as PtpHandle, OperationCode, PtpSession,
    ReceiveStream, ResponseCode, StorageId as PtpStorageId,
};
use crate::PtpError;
use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::Duration;

/// The PTP-over-USB implementation of [`MtpBackend`].
pub(crate) struct UsbBackend {
    session: Arc<PtpSession>,
    /// Neutral device identity, derived once at open.
    device_info: DeviceInfo,
    /// Neutral capabilities, derived once at open.
    capabilities: Capabilities,
}

impl UsbBackend {
    /// Build the backend from an open session, deriving neutral info/capabilities once.
    pub(crate) fn new(session: Arc<PtpSession>, ptp_info: PtpDeviceInfo) -> Self {
        let device_info = DeviceInfo::from_ptp(&ptp_info);
        let capabilities = Capabilities::from_ptp_device_info(&ptp_info);
        Self {
            session,
            device_info,
            capabilities,
        }
    }

    /// Resolve the object-handle list for a listing, applying the root-listing quirks.
    ///
    /// Returns `(handles, filter)` in PTP terms. Mirrors the historical `list_objects_stream`
    /// fast-path/fallback logic exactly, just factored out so the streaming body can be built from
    /// it.
    async fn resolve_listing(
        &self,
        storage: PtpStorageId,
        parent: Option<PtpHandle>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Vec<PtpHandle>, ParentFilter), PtpError> {
        // For root listings, try parent=0xFFFFFFFF first. Many devices (Android, Kindle, others)
        // return only root-level handles for this value, while parent=0 returns every object on the
        // storage. Fall back to parent=0 only when the device rejects 0xFFFFFFFF with an error.
        if parent.is_none() {
            if let Ok(handles) = self
                .session
                .get_object_handles(storage, None, Some(PtpHandle::ALL))
                .await
            {
                return Ok((handles, ParentFilter::AndroidRoot));
            }
            // 0xFFFFFFFF rejected; fall through to parent=0 path.
        }

        bail_if_cancelled(cancel)?;

        let result = self.session.get_object_handles(storage, None, parent).await;

        match result {
            Ok(handles) => {
                let filter = ParentFilter::Exact(parent.unwrap_or(PtpHandle::ROOT));
                Ok((handles, filter))
            }
            Err(PtpError::Protocol {
                code: ResponseCode::InvalidObjectHandle,
                ..
            }) if parent.is_none() => {
                // Samsung fallback: recursive listing filtered to root items.
                let handles = self
                    .session
                    .get_object_handles(storage, None, Some(PtpHandle::ALL))
                    .await?;
                Ok((handles, ParentFilter::AndroidRoot))
            }
            Err(e) => Err(e),
        }
    }
}

/// How to filter objects by parent handle during a listing (PTP terms).
#[derive(Clone, Copy)]
enum ParentFilter {
    /// Accept objects whose parent matches exactly.
    Exact(PtpHandle),
    /// Android root: accept parent 0 or 0xFFFFFFFF.
    AndroidRoot,
}

impl ParentFilter {
    fn accepts(self, parent: PtpHandle) -> bool {
        match self {
            ParentFilter::Exact(expected) => parent == expected,
            ParentFilter::AndroidRoot => parent.0 == 0 || parent.0 == 0xFFFF_FFFF,
        }
    }
}

/// State threaded through the listing `unfold` stream.
struct ListingState {
    session: Arc<PtpSession>,
    handles: Vec<PtpHandle>,
    cursor: usize,
    filter: ParentFilter,
    cancel: Option<CancelToken>,
}

/// The USB streaming-download body: a [`ReceiveStream`] with neutral error conversion.
struct UsbDownloadBody {
    stream: ReceiveStream,
}

#[async_trait]
impl DownloadBody for UsbDownloadBody {
    async fn next_chunk(&mut self) -> Option<Result<Bytes, Error>> {
        self.stream
            .next_chunk()
            .await
            .map(|r| r.map_err(Error::from))
    }

    async fn cancel(&mut self, idle_timeout: Duration) -> Result<(), Error> {
        self.stream.cancel(idle_timeout).await.map_err(Error::from)
    }
}

#[async_trait]
impl MtpBackend for UsbBackend {
    fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    async fn storages(&self) -> Result<Vec<StorageInfo>, Error> {
        let ids = self.session.get_storage_ids().await?;
        let mut storages = Vec::with_capacity(ids.len());
        for id in ids {
            let info = self.session.get_storage_info(id).await?;
            let mut neutral = StorageInfo::from_ptp(&info);
            neutral.id = id.into();
            storages.push(neutral);
        }
        Ok(storages)
    }

    async fn storage_info(&self, storage: StorageId) -> Result<StorageInfo, Error> {
        let info = self.session.get_storage_info(storage.to_ptp()).await?;
        let mut neutral = StorageInfo::from_ptp(&info);
        neutral.id = storage;
        Ok(neutral)
    }

    async fn list(
        &self,
        storage: StorageId,
        parent: Option<ObjectHandle>,
        cancel: Option<&CancelToken>,
    ) -> Result<BackendListing, Error> {
        bail_if_cancelled(cancel)?;

        let (handles, filter) = self
            .resolve_listing(storage.to_ptp(), parent.map(ObjectHandle::to_ptp), cancel)
            .await?;

        let total = handles.len();
        let state = ListingState {
            session: Arc::clone(&self.session),
            handles,
            cursor: 0,
            filter,
            cancel: cancel.cloned(),
        };

        let items = futures::stream::unfold(state, |mut state| async move {
            loop {
                if state.cursor >= state.handles.len() {
                    return None;
                }

                // Cooperative cancel check before issuing the per-handle USB roundtrip. On a
                // 1k-photo listing this is the actual stop point.
                if let Err(e) = bail_if_cancelled(state.cancel.as_ref()) {
                    return Some((Err(Error::from(e)), state));
                }

                let handle = state.handles[state.cursor];
                state.cursor += 1;

                let mut info = match state.session.get_object_info_full(handle).await {
                    Ok(info) => info,
                    Err(e) => return Some((Err(Error::from(e)), state)),
                };
                info.handle = handle;

                if !state.filter.accepts(info.parent) {
                    continue;
                }

                return Some((Ok(ObjectInfo::from_ptp(info)), state));
            }
        });

        Ok(BackendListing {
            total,
            items: Box::pin(items),
        })
    }

    async fn object_info(&self, obj: ObjectHandle) -> Result<ObjectInfo, Error> {
        let mut info = self.session.get_object_info_full(obj.to_ptp()).await?;
        info.handle = obj.to_ptp();
        Ok(ObjectInfo::from_ptp(info))
    }

    async fn download(
        &self,
        obj: ObjectHandle,
        range: ByteRange,
    ) -> Result<BackendDownload, Error> {
        let info = self.session.get_object_info_full(obj.to_ptp()).await?;
        let size = info.size;
        let offset = range.offset();

        if offset > size {
            return Err(Error::invalid_data(format!(
                "download offset {offset} is past the object size {size}"
            )));
        }

        let stream = match range {
            ByteRange::Full => {
                // Whole file via GetObject (the historical fast path; no offset machinery).
                self.session
                    .execute_with_receive_stream(OperationCode::GetObject, &[obj.to_ptp().0])
                    .await?
            }
            ByteRange::From(_) | ByteRange::Range { .. } => {
                // Offset/range read via GetPartialObject64. `max_bytes` is a u32, so a single call
                // requests at most u32::MAX bytes from the offset; a larger tail is fetched across
                // multiple resumes. The 64-bit offset is what lets a resume start past 4 GB.
                let remaining = size - offset;
                let want = match range {
                    ByteRange::Range { len, .. } => remaining.min(len),
                    _ => remaining,
                };
                let max_bytes = u32::try_from(want).unwrap_or(u32::MAX);
                let offset_lo = offset as u32;
                let offset_hi = (offset >> 32) as u32;
                self.session
                    .execute_with_receive_stream(
                        OperationCode::GetPartialObject64,
                        &[obj.to_ptp().0, offset_lo, offset_hi, max_bytes],
                    )
                    .await?
            }
        };

        Ok(BackendDownload {
            // Report the full object size so a resumed/ranged download's progress/ETA stays
            // anchored to the whole file, not just this segment.
            size,
            body: Box::new(UsbDownloadBody { stream }),
        })
    }

    async fn read_range(
        &self,
        obj: ObjectHandle,
        offset: u64,
        len: Option<u32>,
    ) -> Result<Vec<u8>, Error> {
        match len {
            // Whole object: GetObject buffers the lot.
            None if offset == 0 => Ok(self.session.get_object(obj.to_ptp()).await?),
            // Tail from an offset with no explicit length: ask for as much as one call allows.
            None => Ok(self
                .session
                .get_partial_object_64(obj.to_ptp(), offset, u32::MAX)
                .await?),
            Some(len) => Ok(self
                .session
                .get_partial_object_64(obj.to_ptp(), offset, len)
                .await?),
        }
    }

    async fn thumbnail(&self, obj: ObjectHandle) -> Result<Vec<u8>, Error> {
        Ok(self.session.get_thumb(obj.to_ptp()).await?)
    }

    async fn upload(
        &self,
        storage: StorageId,
        parent: Option<ObjectHandle>,
        info: NewObjectInfo,
        data: UploadStream<'_>,
        progress: Option<ProgressFn>,
    ) -> Result<ObjectHandle, UploadError> {
        let total_size = info.size;
        let object_info = info.to_object_info();
        let parent_handle = parent.map(ObjectHandle::to_ptp).unwrap_or(PtpHandle::ROOT);

        // Phase 1: SendObjectInfo. No object exists yet, so a failure here has no partial to
        // surface.
        let (_, _, handle) = self
            .session
            .send_object_info(storage.to_ptp(), parent_handle, &object_info)
            .await
            .map_err(|source| UploadError {
                source: source.into(),
                partial: None,
            })?;

        // Wrap the stream to report progress and support cancellation.
        let mut bytes_sent = 0u64;
        let mut progress = progress;
        let progress_stream = data.map(move |chunk_result| {
            let chunk = chunk_result?;
            bytes_sent += chunk.len() as u64;
            if let Some(cb) = progress.as_mut() {
                let p = Progress {
                    bytes_transferred: bytes_sent,
                    total_bytes: Some(total_size),
                };
                if let ControlFlow::Break(()) = cb(p) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "cancelled",
                    ));
                }
            }
            Ok(chunk)
        });

        // Phase 2: SendObject. The object already exists on the device, so any failure (genuine
        // error or cancellation) surfaces the handle as `partial` for the caller to delete or
        // resume. We do NOT delete it here.
        self.session
            .send_object_stream(total_size, progress_stream)
            .await
            .map_err(|e| match &e {
                PtpError::Io(io_err) if io_err.kind() == std::io::ErrorKind::Interrupted => {
                    Error::Cancelled
                }
                _ => Error::from(e),
            })
            .map_err(|source| UploadError {
                source,
                partial: Some(handle.into()),
            })?;

        Ok(handle.into())
    }

    async fn create_folder(
        &self,
        storage: StorageId,
        parent: Option<ObjectHandle>,
        name: &str,
    ) -> Result<ObjectHandle, Error> {
        let info = NewObjectInfo::folder(name);
        let object_info = info.to_object_info();
        let parent_handle = parent.map(ObjectHandle::to_ptp).unwrap_or(PtpHandle::ROOT);

        let (_, _, handle) = self
            .session
            .send_object_info(storage.to_ptp(), parent_handle, &object_info)
            .await?;
        Ok(handle.into())
    }

    async fn delete(&self, obj: ObjectHandle, cancel: Option<&CancelToken>) -> Result<(), Error> {
        bail_if_cancelled(cancel)?;
        Ok(self.session.delete_object(obj.to_ptp()).await?)
    }

    async fn move_object(
        &self,
        obj: ObjectHandle,
        new_parent: ObjectHandle,
        new_storage: StorageId,
    ) -> Result<(), Error> {
        Ok(self
            .session
            .move_object(obj.to_ptp(), new_storage.to_ptp(), new_parent.to_ptp())
            .await?)
    }

    async fn copy_object(
        &self,
        obj: ObjectHandle,
        new_parent: ObjectHandle,
        new_storage: StorageId,
    ) -> Result<ObjectHandle, Error> {
        let handle = self
            .session
            .copy_object(obj.to_ptp(), new_storage.to_ptp(), new_parent.to_ptp())
            .await?;
        Ok(handle.into())
    }

    async fn rename(&self, obj: ObjectHandle, new_name: &str) -> Result<(), Error> {
        Ok(self.session.rename_object(obj.to_ptp(), new_name).await?)
    }

    async fn next_event(&self) -> Result<DeviceEvent, Error> {
        match self.session.poll_event().await? {
            Some(container) => Ok(DeviceEvent::from_container(&container)),
            None => Err(Error::Timeout),
        }
    }

    async fn close(&self) -> Result<(), Error> {
        // Best-effort CloseSession (the device tolerates a missing one; drop also cleans up).
        let _ = self.session.execute(OperationCode::CloseSession, &[]).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ptp::{
        pack_u16, pack_u32, pack_u32_array, pack_u64, ContainerType, DateTime as PtpDateTime,
        DeviceInfo as PtpDeviceInfo, ObjectFormatCode, ObjectInfo as PtpObjectInfo,
    };
    use crate::transport::mock::MockTransport;

    // -- Protocol-level mock helpers (mirror the session/storage test helpers) ----

    fn mock_transport() -> (Arc<dyn crate::transport::Transport>, Arc<MockTransport>) {
        let mock = Arc::new(MockTransport::new());
        let transport: Arc<dyn crate::transport::Transport> = Arc::clone(&mock) as _;
        (transport, mock)
    }

    fn ok_response(tx_id: u32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(12);
        buf.extend_from_slice(&pack_u32(12));
        buf.extend_from_slice(&pack_u16(ContainerType::Response.to_code()));
        buf.extend_from_slice(&pack_u16(ResponseCode::Ok.into()));
        buf.extend_from_slice(&pack_u32(tx_id));
        buf
    }

    fn error_response(tx_id: u32, code: ResponseCode) -> Vec<u8> {
        let mut buf = Vec::with_capacity(12);
        buf.extend_from_slice(&pack_u32(12));
        buf.extend_from_slice(&pack_u16(ContainerType::Response.to_code()));
        buf.extend_from_slice(&pack_u16(code.into()));
        buf.extend_from_slice(&pack_u32(tx_id));
        buf
    }

    fn data_container(tx_id: u32, code: OperationCode, payload: &[u8]) -> Vec<u8> {
        let len = 12 + payload.len();
        let mut buf = Vec::with_capacity(len);
        buf.extend_from_slice(&pack_u32(len as u32));
        buf.extend_from_slice(&pack_u16(ContainerType::Data.to_code()));
        buf.extend_from_slice(&pack_u16(code.into()));
        buf.extend_from_slice(&pack_u32(tx_id));
        buf.extend_from_slice(payload);
        buf
    }

    /// Build a `UsbBackend` over a mock transport with a given vendor extension descriptor.
    /// Queues the OpenSession response; the caller queues further responses before listing.
    async fn mock_backend(
        transport: Arc<dyn crate::transport::Transport>,
        vendor_extension_desc: &str,
    ) -> UsbBackend {
        let session = Arc::new(PtpSession::open(transport, 1).await.unwrap());
        let ptp_info = PtpDeviceInfo {
            vendor_extension_desc: vendor_extension_desc.to_string(),
            ..PtpDeviceInfo::default()
        };
        UsbBackend::new(session, ptp_info)
    }

    fn object_info_bytes(filename: &str, parent: u32) -> Vec<u8> {
        let info = PtpObjectInfo {
            storage_id: PtpStorageId(1),
            format: ObjectFormatCode::Jpeg,
            parent: PtpHandle(parent),
            filename: filename.to_string(),
            created: Some(PtpDateTime {
                year: 2024,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
            }),
            ..PtpObjectInfo::default()
        };
        info.to_bytes().unwrap()
    }

    fn object_info_bytes_with_size(filename: &str, parent: u32, size: u64) -> Vec<u8> {
        let info = PtpObjectInfo {
            storage_id: PtpStorageId(1),
            format: ObjectFormatCode::Jpeg,
            parent: PtpHandle(parent),
            filename: filename.to_string(),
            size,
            ..PtpObjectInfo::default()
        };
        info.to_bytes().unwrap()
    }

    fn queue_handles(mock: &MockTransport, tx_id: u32, handles: &[u32]) {
        let data = pack_u32_array(handles);
        mock.queue_response(data_container(
            tx_id,
            OperationCode::GetObjectHandles,
            &data,
        ));
        mock.queue_response(ok_response(tx_id));
    }

    fn queue_object_info(mock: &MockTransport, tx_id: u32, filename: &str, parent: u32) {
        let data = object_info_bytes(filename, parent);
        mock.queue_response(data_container(tx_id, OperationCode::GetObjectInfo, &data));
        mock.queue_response(ok_response(tx_id));
    }

    fn queue_object_info_with_size(
        mock: &MockTransport,
        tx_id: u32,
        filename: &str,
        parent: u32,
        size: u64,
    ) {
        let data = object_info_bytes_with_size(filename, parent, size);
        mock.queue_response(data_container(tx_id, OperationCode::GetObjectInfo, &data));
        mock.queue_response(ok_response(tx_id));
    }

    fn queue_object_size_prop(mock: &MockTransport, tx_id: u32, size: u64) {
        let payload = pack_u64(size);
        mock.queue_response(data_container(
            tx_id,
            OperationCode::GetObjectPropValue,
            &payload,
        ));
        mock.queue_response(ok_response(tx_id));
    }

    /// Drive a `BackendListing` to a Vec, surfacing the first error.
    async fn collect(mut listing: BackendListing) -> Result<Vec<ObjectInfo>, Error> {
        let mut out = Vec::new();
        while let Some(item) = listing.items.next().await {
            out.push(item?);
        }
        Ok(out)
    }

    const SID: StorageId = StorageId(1);

    // -- Root listing fast-path / filtering --------------------------------------

    #[tokio::test]
    async fn list_root_fast_path_filters_non_root() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));
        queue_handles(&mock, 1, &[10, 20, 30]);
        queue_object_info(&mock, 2, "root_file.jpg", 0); // parent=ROOT, included
        queue_object_info(&mock, 3, "nested.jpg", 99); // filtered out
        queue_object_info(&mock, 4, "another_root.txt", 0); // included

        let backend = mock_backend(transport, "").await;
        let listing = backend.list(SID, None, None).await.unwrap();
        assert_eq!(listing.total, 3);
        let objs = collect(listing).await.unwrap();
        assert_eq!(objs.len(), 2);
        assert_eq!(objs[0].filename, "root_file.jpg");
        assert_eq!(objs[1].filename, "another_root.txt");
    }

    #[tokio::test]
    async fn list_root_accepts_both_parent_values() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));
        queue_handles(&mock, 1, &[10, 20, 30]);
        queue_object_info(&mock, 2, "dcim", 0); // parent=0
        queue_object_info(&mock, 3, "download", 0xFFFFFFFF); // parent=ALL
        queue_object_info(&mock, 4, "nested", 42); // not root

        let backend = mock_backend(transport, "").await;
        let objs = collect(backend.list(SID, None, None).await.unwrap())
            .await
            .unwrap();
        assert_eq!(objs.len(), 2);
        assert_eq!(objs[0].filename, "dcim");
        assert_eq!(objs[1].filename, "download");
    }

    #[tokio::test]
    async fn list_empty_directory() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));
        queue_handles(&mock, 1, &[]);

        let backend = mock_backend(transport, "").await;
        let listing = backend.list(SID, None, None).await.unwrap();
        assert_eq!(listing.total, 0);
        assert!(collect(listing).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_subfolder_uses_exact_filter() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));
        let parent = 42u32;
        queue_handles(&mock, 1, &[100, 101]);
        queue_object_info(&mock, 2, "IMG_001.jpg", parent);
        queue_object_info(&mock, 3, "IMG_002.jpg", parent);

        let backend = mock_backend(transport, "").await;
        let objs = collect(
            backend
                .list(SID, Some(ObjectHandle(u64::from(parent))), None)
                .await
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(objs.len(), 2);
        assert_eq!(objs[0].filename, "IMG_001.jpg");
    }

    #[tokio::test]
    async fn list_propagates_mid_listing_error() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));
        queue_handles(&mock, 1, &[10, 20]);
        queue_object_info(&mock, 2, "good.jpg", 0);
        mock.queue_response(error_response(3, ResponseCode::InvalidObjectHandle));

        let backend = mock_backend(transport, "").await;
        let mut listing = backend.list(SID, None, None).await.unwrap();
        let first = listing.items.next().await.unwrap().unwrap();
        assert_eq!(first.filename, "good.jpg");
        assert!(listing.items.next().await.unwrap().is_err());
    }

    // -- Root fallback (Samsung) -------------------------------------------------

    #[tokio::test]
    async fn list_root_falls_back_on_error() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));
        // Fast path (0xFFFFFFFF) rejected.
        mock.queue_response(error_response(1, ResponseCode::InvalidObjectHandle));
        // Fallback (parent=0) succeeds.
        queue_handles(&mock, 2, &[10, 20]);
        queue_object_info(&mock, 3, "root.jpg", 0);
        queue_object_info(&mock, 4, "nested.jpg", 99); // filtered by Exact(ROOT)

        let backend = mock_backend(transport, "").await;
        let objs = collect(backend.list(SID, None, None).await.unwrap())
            .await
            .unwrap();
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0].filename, "root.jpg");
    }

    #[tokio::test]
    async fn list_root_empty_is_not_fallback() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));
        queue_handles(&mock, 1, &[]); // fast path returns empty

        let backend = mock_backend(transport, "").await;
        let listing = backend.list(SID, None, None).await.unwrap();
        assert_eq!(listing.total, 0);
    }

    // -- >4 GB size resolution ---------------------------------------------------

    #[tokio::test]
    async fn object_info_resolves_saturated_size() {
        const REAL_SIZE: u64 = 5 * 1024 * 1024 * 1024;
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));
        queue_object_info_with_size(&mock, 1, "big.mkv", 0, REAL_SIZE);
        queue_object_size_prop(&mock, 2, REAL_SIZE);

        let backend = mock_backend(transport, "").await;
        let info = backend.object_info(ObjectHandle(42)).await.unwrap();
        assert_eq!(info.size, REAL_SIZE);
    }

    #[tokio::test]
    async fn object_info_skips_lookup_when_size_fits_u32() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));
        queue_object_info_with_size(&mock, 1, "small.jpg", 0, 1_000_000);

        let backend = mock_backend(transport, "").await;
        let info = backend.object_info(ObjectHandle(42)).await.unwrap();
        assert_eq!(info.size, 1_000_000);
    }

    #[tokio::test]
    async fn object_info_falls_back_when_prop_lookup_fails() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));
        queue_object_info_with_size(&mock, 1, "big.mkv", 0, 8 * 1024 * 1024 * 1024);
        mock.queue_response(error_response(2, ResponseCode::OperationNotSupported));

        let backend = mock_backend(transport, "").await;
        let info = backend.object_info(ObjectHandle(42)).await.unwrap();
        assert_eq!(info.size, u64::from(u32::MAX));
    }

    // -- Cancellation ------------------------------------------------------------

    #[tokio::test]
    async fn list_cancel_before_first_handle_bails() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));
        queue_handles(&mock, 1, &[10, 20, 30]);

        let backend = mock_backend(transport, "").await;
        let cancel = CancelToken::new();
        let mut listing = backend.list(SID, None, Some(&cancel)).await.unwrap();
        assert_eq!(listing.total, 3);
        cancel.cancel();
        let first = listing.items.next().await.expect("expected Some(Err)");
        assert!(matches!(first, Err(Error::Cancelled)));
    }

    #[tokio::test]
    async fn list_cancel_mid_listing_bails_at_next_boundary() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));
        queue_handles(&mock, 1, &[10, 20, 30]);
        queue_object_info(&mock, 2, "first.jpg", 0);

        let backend = mock_backend(transport, "").await;
        let cancel = CancelToken::new();
        let mut listing = backend.list(SID, None, Some(&cancel)).await.unwrap();
        let first = listing.items.next().await.unwrap().unwrap();
        assert_eq!(first.filename, "first.jpg");
        cancel.cancel();
        let second = listing.items.next().await.expect("expected Some(Err)");
        assert!(matches!(second, Err(Error::Cancelled)));
    }

    #[tokio::test]
    async fn delete_with_cancel_bails_before_request() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));

        let backend = mock_backend(transport, "").await;
        let cancel = CancelToken::new();
        cancel.cancel();
        let result = backend.delete(ObjectHandle(1), Some(&cancel)).await;
        assert!(matches!(result, Err(Error::Cancelled)));
    }

    #[tokio::test]
    async fn delete_no_token_runs_normally() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(0));
        mock.queue_response(ok_response(1)); // DeleteObject

        let backend = mock_backend(transport, "").await;
        assert!(backend.delete(ObjectHandle(1), None).await.is_ok());
    }
}
