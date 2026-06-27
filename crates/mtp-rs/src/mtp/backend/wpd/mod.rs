//! The Windows WPD-over-COM backend (`cfg(windows)`).
//!
//! Implements the backend-neutral [`MtpBackend`](crate::mtp::backend::MtpBackend) trait against the
//! Windows Portable Devices COM API. WPD is *not* a USB transport — it speaks MTP for us and exposes
//! a high-level object model — so this backend is a sibling to [`UsbBackend`](super::usb::UsbBackend),
//! not another `Transport`. See `docs/windows-wpd-backend-plan.md`.
//!
//! ## Threading
//!
//! WPD COM pointers are apartment-affine and `!Send`/`!Sync`. Rather than wrap them in `unsafe Send`,
//! one dedicated [`std::thread`] per open device (`actor.rs`) owns *all* the COM interface pointers,
//! `CoInitializeEx`es an MTA, and processes one request at a time off a channel. [`WpdBackend`] holds
//! only channel senders, so it is `Send + Sync` with **zero `unsafe`** in the public path.

mod actor;
mod com;
mod consts;
mod ids;
mod props;

use crate::cancel::CancelToken;
use crate::mtp::backend::{
    BackendDownload, BackendListing, ByteRange, DownloadBody, MtpBackend, ProgressFn, UploadStream,
};
use crate::mtp::object::NewObjectInfo;
use crate::mtp::{
    Capabilities, DeviceEvent, DeviceInfo, Error, ObjectHandle, ObjectInfo, StorageId, StorageInfo,
    UploadError,
};
use actor::{OpenSpec, WpdHandle};
use async_trait::async_trait;
use bytes::Bytes;
use futures::channel::mpsc;
use futures::StreamExt;
use std::time::Duration;

/// The Windows WPD-over-COM backend. Holds only the worker handle (channel ends) plus the device's
/// identity/capabilities cached at open, so it is `Send + Sync` with no `unsafe`.
pub(crate) struct WpdBackend {
    handle: WpdHandle,
    device_info: DeviceInfo,
    capabilities: Capabilities,
}

impl WpdBackend {
    /// Open the first WPD device Windows enumerates.
    pub(crate) async fn open_first() -> Result<Self, Error> {
        Self::spawn(OpenSpec::First).await
    }

    /// Open the WPD device whose serial number matches.
    pub(crate) async fn open_by_serial(serial: &str) -> Result<Self, Error> {
        Self::spawn(OpenSpec::Serial(serial.to_string())).await
    }

    async fn spawn(spec: OpenSpec) -> Result<Self, Error> {
        let (handle, device_info, capabilities) = WpdHandle::spawn(spec).await?;
        Ok(Self {
            handle,
            device_info,
            capabilities,
        })
    }
}

/// The WPD streaming-download body: chunks arrive over a bounded channel from the worker thread.
struct WpdDownloadBody {
    data: mpsc::Receiver<Result<Bytes, Error>>,
}

#[async_trait]
impl DownloadBody for WpdDownloadBody {
    async fn next_chunk(&mut self) -> Option<Result<Bytes, Error>> {
        self.data.next().await
    }

    async fn cancel(&mut self, _idle_timeout: Duration) -> Result<(), Error> {
        // Close + drain the receiver: the worker's next `send` then fails, which stops its read
        // loop and releases the `IStream`. WPD cancel is "stop reading + Release" — no SIC drain.
        self.data.close();
        while self.data.next().await.is_some() {}
        Ok(())
    }
}

#[async_trait]
impl MtpBackend for WpdBackend {
    fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    async fn storages(&self) -> Result<Vec<StorageInfo>, Error> {
        self.handle.call(actor::Request::Storages).await
    }

    async fn storage_info(&self, storage: StorageId) -> Result<StorageInfo, Error> {
        self.handle
            .call(|reply| actor::Request::StorageInfo(storage, reply))
            .await
    }

    async fn list(
        &self,
        storage: StorageId,
        parent: Option<ObjectHandle>,
        cancel: Option<&CancelToken>,
    ) -> Result<BackendListing, Error> {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(Error::Cancelled);
        }
        let objs = self
            .handle
            .call(|reply| actor::Request::List {
                storage,
                parent,
                cancel: cancel.cloned(),
                reply,
            })
            .await?;
        let total = objs.len();
        let items = futures::stream::iter(objs.into_iter().map(Ok::<ObjectInfo, Error>)).boxed();
        Ok(BackendListing { total, items })
    }

    async fn object_info(&self, obj: ObjectHandle) -> Result<ObjectInfo, Error> {
        self.handle
            .call(|reply| actor::Request::ObjectInfo(obj, reply))
            .await
    }

    async fn download(
        &self,
        obj: ObjectHandle,
        range: ByteRange,
    ) -> Result<BackendDownload, Error> {
        let start = self
            .handle
            .call(|reply| actor::Request::Download { obj, range, reply })
            .await?;
        Ok(BackendDownload {
            size: start.size,
            body: Box::new(WpdDownloadBody { data: start.data }),
        })
    }

    async fn read_range(
        &self,
        obj: ObjectHandle,
        offset: u64,
        len: Option<u32>,
    ) -> Result<Vec<u8>, Error> {
        self.handle
            .call(|reply| actor::Request::ReadRange {
                obj,
                offset,
                len,
                reply,
            })
            .await
    }

    async fn thumbnail(&self, _obj: ObjectHandle) -> Result<Vec<u8>, Error> {
        // TODO(phase-3+): WPD_RESOURCE_THUMBNAIL via GetStream. Reported unsupported for now.
        Err(Error::Unsupported)
    }

    // ---- write path: Phase 3 (stubbed Unsupported so the read path is testable now) ------------

    async fn upload(
        &self,
        _storage: StorageId,
        _parent: Option<ObjectHandle>,
        _info: NewObjectInfo,
        _data: UploadStream<'_>,
        _progress: Option<ProgressFn<'_>>,
    ) -> Result<ObjectHandle, UploadError> {
        Err(UploadError {
            source: Error::Unsupported,
            partial: None,
        })
    }

    async fn create_folder(
        &self,
        _storage: StorageId,
        _parent: Option<ObjectHandle>,
        _name: &str,
    ) -> Result<ObjectHandle, Error> {
        Err(Error::Unsupported)
    }

    async fn delete(&self, _obj: ObjectHandle, _cancel: Option<&CancelToken>) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    async fn move_object(
        &self,
        _obj: ObjectHandle,
        _new_parent: ObjectHandle,
        _new_storage: StorageId,
    ) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    async fn copy_object(
        &self,
        _obj: ObjectHandle,
        _new_parent: ObjectHandle,
        _new_storage: StorageId,
    ) -> Result<ObjectHandle, Error> {
        Err(Error::Unsupported)
    }

    async fn rename(&self, _obj: ObjectHandle, _new_name: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    async fn next_event(&self) -> Result<DeviceEvent, Error> {
        // Events are Phase 4 (deferred). See docs/windows-wpd-backend-plan.md.
        Err(Error::Unsupported)
    }

    async fn close(&self) -> Result<(), Error> {
        self.handle.shutdown();
        Ok(())
    }
}
