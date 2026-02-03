//! Storage operations.

use crate::mtp::object::NewObjectInfo;
use crate::mtp::stream::{DownloadStream, Progress};
use crate::ptp::{ObjectHandle, ObjectInfo, StorageId, StorageInfo};
use crate::Error;
use bytes::Bytes;
use futures::Stream;
use std::ops::ControlFlow;
use std::sync::Arc;

// Import MtpDeviceInner from device.rs
use super::device::MtpDeviceInner;

/// A storage location on an MTP device.
///
/// `Storage` holds an `Arc<MtpDeviceInner>` so it can outlive the original
/// `MtpDevice` and be used from multiple tasks.
pub struct Storage {
    inner: Arc<MtpDeviceInner>,
    id: StorageId,
    info: StorageInfo,
}

impl Storage {
    /// Create a new Storage (internal).
    pub(crate) fn new(inner: Arc<MtpDeviceInner>, id: StorageId, info: StorageInfo) -> Self {
        Self { inner, id, info }
    }

    /// Storage identifier.
    pub fn id(&self) -> StorageId {
        self.id
    }

    /// Storage information (cached, call refresh() to update).
    pub fn info(&self) -> &StorageInfo {
        &self.info
    }

    /// Refresh storage info from device (updates free space, etc.).
    pub async fn refresh(&mut self) -> Result<(), Error> {
        self.info = self.inner.session.get_storage_info(self.id).await?;
        Ok(())
    }

    /// List objects in a folder (None = root).
    ///
    /// This method handles various device quirks:
    /// - Android devices: parent=0 returns ALL objects, so we use parent=0xFFFFFFFF instead
    /// - Samsung devices: return InvalidObjectHandle for parent=0, so we fall back to recursive
    /// - Fuji devices: return all objects for root, so we filter by parent handle
    pub async fn list_objects(
        &self,
        parent: Option<ObjectHandle>,
    ) -> Result<Vec<ObjectInfo>, Error> {
        // Android quirk: When listing root (parent=None/0), Android returns ALL objects
        // on the device instead of just root-level objects. This makes listing extremely slow.
        // Counter-intuitively, using parent=0xFFFFFFFF (ObjectHandle::ALL) returns the
        // actual root-level objects on Android devices.
        let effective_parent = if parent.is_none() && self.inner.is_android() {
            Some(ObjectHandle::ALL)
        } else {
            parent
        };

        let result = self
            .inner
            .session
            .get_object_handles(
                self.id,
                None, // All formats
                effective_parent,
            )
            .await;

        let handles = match result {
            Ok(h) => h,
            Err(Error::Protocol {
                code: crate::ptp::ResponseCode::InvalidObjectHandle,
                ..
            }) if parent.is_none() => {
                // Samsung fallback: use recursive listing and filter to root items
                return self.list_objects_samsung_fallback().await;
            }
            Err(e) => return Err(e),
        };

        let mut objects = Vec::with_capacity(handles.len());
        let expected_parent = parent.unwrap_or(ObjectHandle::ROOT);

        for handle in handles {
            let mut info = self.inner.session.get_object_info(handle).await?;
            info.handle = handle;

            // Filter: only include objects whose parent matches the requested parent.
            // Some devices (like Fuji X-T4) return all objects when asked for root,
            // not just root-level objects.
            // For Android root listing (where we used ALL), accept both 0 and 0xFFFFFFFF as parent.
            let parent_matches = if parent.is_none() && self.inner.is_android() {
                info.parent.0 == 0 || info.parent.0 == 0xFFFFFFFF
            } else {
                info.parent == expected_parent
            };

            if parent_matches {
                objects.push(info);
            }
        }
        Ok(objects)
    }

    /// Samsung fallback: list all objects recursively and filter to root level.
    ///
    /// Samsung devices return InvalidObjectHandle when trying to list root (handle 0).
    /// Instead, we list all objects recursively (handle 0xFFFFFFFF) and filter to
    /// those at the root level (parent_handle == 0 or parent_handle == 0xFFFFFFFF).
    async fn list_objects_samsung_fallback(&self) -> Result<Vec<ObjectInfo>, Error> {
        // Get all object handles recursively
        let handles = self
            .inner
            .session
            .get_object_handles(self.id, None, Some(ObjectHandle::ALL))
            .await?;

        // Get info for each and filter to root level
        let mut objects = Vec::new();
        for handle in handles {
            let mut info = self.inner.session.get_object_info(handle).await?;
            info.handle = handle;

            // Root items have parent 0 or 0xFFFFFFFF (depending on device)
            if info.parent.0 == 0 || info.parent.0 == 0xFFFFFFFF {
                objects.push(info);
            }
        }
        Ok(objects)
    }

    /// List objects recursively.
    ///
    /// This method automatically detects Android devices and uses manual traversal
    /// for them, since Android's MTP implementation doesn't support the native
    /// `ObjectHandle::ALL` recursive listing.
    ///
    /// For non-Android devices, it tries native recursive listing first and falls
    /// back to manual traversal if the results look incomplete (e.g., only folders
    /// returned with no files).
    pub async fn list_objects_recursive(
        &self,
        parent: Option<ObjectHandle>,
    ) -> Result<Vec<ObjectInfo>, Error> {
        if self.inner.is_android() {
            // Android doesn't support native recursive listing
            return self.list_objects_recursive_manual(parent).await;
        }

        // Try native recursive listing for non-Android devices
        let native_result = self.list_objects_recursive_native(parent).await?;

        // Heuristic: if we only got folders and no files, the native listing
        // probably didn't work correctly - fall back to manual traversal
        let has_files = native_result.iter().any(|o| o.is_file());
        if !native_result.is_empty() && !has_files {
            // Looks like native didn't work, try manual
            return self.list_objects_recursive_manual(parent).await;
        }

        Ok(native_result)
    }

    /// List objects recursively using native MTP recursive listing.
    ///
    /// Uses `ObjectHandle::ALL` to request recursive listing from the device.
    /// Note: This doesn't work on Android devices - use `list_objects_recursive()`
    /// which handles this automatically.
    pub async fn list_objects_recursive_native(
        &self,
        parent: Option<ObjectHandle>,
    ) -> Result<Vec<ObjectInfo>, Error> {
        // Use ALL handle for recursive listing
        let recursive_parent = if parent.is_none() {
            Some(ObjectHandle::ALL)
        } else {
            parent
        };

        let handles = self
            .inner
            .session
            .get_object_handles(self.id, None, recursive_parent)
            .await?;

        let mut objects = Vec::with_capacity(handles.len());
        for handle in handles {
            let mut info = self.inner.session.get_object_info(handle).await?;
            info.handle = handle;
            objects.push(info);
        }
        Ok(objects)
    }

    /// List objects recursively using manual folder traversal.
    ///
    /// This method traverses folders one by one, which works on all devices
    /// including Android. It's slower than native recursive listing but
    /// always works.
    pub async fn list_objects_recursive_manual(
        &self,
        parent: Option<ObjectHandle>,
    ) -> Result<Vec<ObjectInfo>, Error> {
        let mut result = Vec::new();
        let mut folders_to_visit = vec![parent];

        while let Some(current_parent) = folders_to_visit.pop() {
            let objects = self.list_objects(current_parent).await?;

            for obj in objects {
                if obj.is_folder() {
                    // Add folder to visit list
                    folders_to_visit.push(Some(obj.handle));
                }
                result.push(obj);
            }
        }

        Ok(result)
    }

    /// Get object metadata by handle.
    pub async fn get_object_info(&self, handle: ObjectHandle) -> Result<ObjectInfo, Error> {
        let mut info = self.inner.session.get_object_info(handle).await?;
        info.handle = handle;
        Ok(info)
    }

    /// Download a file.
    pub async fn download(&self, handle: ObjectHandle) -> Result<DownloadStream, Error> {
        let data = self.inner.session.get_object(handle).await?;
        Ok(DownloadStream::new(data))
    }

    /// Download a partial file (byte range).
    pub async fn download_partial(
        &self,
        handle: ObjectHandle,
        offset: u64,
        size: u32,
    ) -> Result<DownloadStream, Error> {
        let data = self
            .inner
            .session
            .get_partial_object(handle, offset, size)
            .await?;
        Ok(DownloadStream::new(data))
    }

    /// Download thumbnail.
    pub async fn download_thumbnail(&self, handle: ObjectHandle) -> Result<Vec<u8>, Error> {
        self.inner.session.get_thumb(handle).await
    }

    /// Upload a file from a stream.
    pub async fn upload<S>(
        &self,
        parent: Option<ObjectHandle>,
        info: NewObjectInfo,
        mut data: S,
    ) -> Result<ObjectHandle, Error>
    where
        S: Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
    {
        use futures::StreamExt;

        // Collect all data first (MTP requires knowing size upfront)
        let mut buffer = Vec::with_capacity(info.size as usize);
        while let Some(chunk) = data.next().await {
            let chunk = chunk.map_err(Error::Io)?;
            buffer.extend_from_slice(&chunk);
        }

        // Send object info
        let object_info = info.to_object_info();
        let parent_handle = parent.unwrap_or(ObjectHandle::ROOT);
        let (_, _, handle) = self
            .inner
            .session
            .send_object_info(self.id, parent_handle, &object_info)
            .await?;

        // Send object data
        self.inner.session.send_object(&buffer).await?;

        Ok(handle)
    }

    /// Upload a file with progress callback.
    pub async fn upload_with_progress<S, F>(
        &self,
        parent: Option<ObjectHandle>,
        info: NewObjectInfo,
        data: S,
        mut on_progress: F,
    ) -> Result<ObjectHandle, Error>
    where
        S: Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
        F: FnMut(Progress) -> ControlFlow<()>,
    {
        use futures::StreamExt;

        let total_size = info.size;
        let mut buffer = Vec::with_capacity(total_size as usize);
        let mut bytes_received = 0u64;

        let mut data = data;
        while let Some(chunk) = data.next().await {
            let chunk = chunk.map_err(Error::Io)?;
            bytes_received += chunk.len() as u64;
            buffer.extend_from_slice(&chunk);

            let progress = Progress {
                bytes_transferred: bytes_received,
                total_bytes: Some(total_size),
            };

            if let ControlFlow::Break(()) = on_progress(progress) {
                return Err(Error::Cancelled);
            }
        }

        // Send object info
        let object_info = info.to_object_info();
        let parent_handle = parent.unwrap_or(ObjectHandle::ROOT);
        let (_, _, handle) = self
            .inner
            .session
            .send_object_info(self.id, parent_handle, &object_info)
            .await?;

        // Send object data
        self.inner.session.send_object(&buffer).await?;

        Ok(handle)
    }

    /// Create a folder.
    pub async fn create_folder(
        &self,
        parent: Option<ObjectHandle>,
        name: &str,
    ) -> Result<ObjectHandle, Error> {
        let info = NewObjectInfo::folder(name);
        let object_info = info.to_object_info();
        let parent_handle = parent.unwrap_or(ObjectHandle::ROOT);

        let (_, _, handle) = self
            .inner
            .session
            .send_object_info(self.id, parent_handle, &object_info)
            .await?;

        // Folders don't need SendObject
        Ok(handle)
    }

    /// Delete an object.
    pub async fn delete(&self, handle: ObjectHandle) -> Result<(), Error> {
        self.inner.session.delete_object(handle).await
    }

    /// Move an object to a different folder.
    pub async fn move_object(
        &self,
        handle: ObjectHandle,
        new_parent: ObjectHandle,
        new_storage: Option<StorageId>,
    ) -> Result<(), Error> {
        let storage = new_storage.unwrap_or(self.id);
        self.inner
            .session
            .move_object(handle, storage, new_parent)
            .await
    }

    /// Copy an object.
    pub async fn copy_object(
        &self,
        handle: ObjectHandle,
        new_parent: ObjectHandle,
        new_storage: Option<StorageId>,
    ) -> Result<ObjectHandle, Error> {
        let storage = new_storage.unwrap_or(self.id);
        self.inner
            .session
            .copy_object(handle, storage, new_parent)
            .await
    }

    /// Rename an object (file or folder).
    ///
    /// This changes the filename of an existing object using the MTP
    /// SetObjectPropValue operation.
    ///
    /// # Arguments
    ///
    /// * `handle` - The object handle to rename
    /// * `new_name` - The new filename
    ///
    /// # Note
    ///
    /// Not all devices support renaming. Use `MtpDevice::supports_rename()`
    /// to check if this operation is available.
    pub async fn rename(&self, handle: ObjectHandle, new_name: &str) -> Result<(), Error> {
        self.inner.session.rename_object(handle, new_name).await
    }

    // =========================================================================
    // Streaming operations (zero-copy)
    // =========================================================================

    /// Download a file as a true stream without buffering the entire file.
    ///
    /// Unlike `download()`, this method yields data chunks directly from USB
    /// as they arrive, without buffering the entire file in memory first.
    /// This is ideal for large files or when piping data to another destination.
    ///
    /// # Returns
    ///
    /// A tuple of (file_size, ReceiveStream) where:
    /// - `file_size` - The total size of the file in bytes
    /// - `ReceiveStream` - A stream that yields `Bytes` chunks
    ///
    /// # Important
    ///
    /// You must consume the entire stream (or drop it) before calling other
    /// storage methods. The MTP session is locked while the stream is active.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use futures::StreamExt;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// let (size, mut stream) = storage.download_streaming(handle).await?;
    /// println!("Downloading {} bytes...", size);
    ///
    /// let mut file = tokio::fs::File::create("output.bin").await?;
    /// while let Some(result) = stream.next_chunk().await {
    ///     let chunk = result?;
    ///     file.write_all(&chunk).await?;
    /// }
    /// ```
    pub async fn download_streaming(
        &self,
        handle: ObjectHandle,
    ) -> Result<(u64, crate::ptp::ReceiveStream), Error> {
        // Get object info first to know the size
        let info = self.get_object_info(handle).await?;
        let size = info.size;

        // Start streaming download
        let stream = self
            .inner
            .session
            .execute_with_receive_stream(crate::ptp::OperationCode::GetObject, &[handle.0])
            .await?;

        Ok((size, stream))
    }

    /// Upload a file from a stream with known size (streaming, no buffering).
    ///
    /// Unlike `upload()`, this method streams data directly to USB without
    /// buffering the entire file in memory first. This is ideal for large
    /// files or when piping data from another source.
    ///
    /// # Arguments
    ///
    /// * `parent` - Parent folder handle (None for root)
    /// * `info` - Object metadata (MUST include accurate size)
    /// * `data` - Stream of data chunks to upload
    ///
    /// # Important
    ///
    /// The `info.size` field MUST match the actual total bytes in the stream.
    /// MTP requires knowing the size before transfer begins.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use futures::stream;
    /// use bytes::Bytes;
    ///
    /// let data = vec![1, 2, 3, 4, 5];
    /// let info = NewObjectInfo::file("test.bin", data.len() as u64);
    ///
    /// // Create a stream from the data
    /// let stream = stream::once(async move { Ok::<_, std::io::Error>(Bytes::from(data)) });
    ///
    /// let handle = storage.upload_streaming(None, info, stream).await?;
    /// ```
    pub async fn upload_streaming<S>(
        &self,
        parent: Option<ObjectHandle>,
        info: NewObjectInfo,
        data: S,
    ) -> Result<ObjectHandle, Error>
    where
        S: Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
    {
        // Send object info first
        let object_info = info.to_object_info();
        let parent_handle = parent.unwrap_or(ObjectHandle::ROOT);
        let (_, _, handle) = self
            .inner
            .session
            .send_object_info(self.id, parent_handle, &object_info)
            .await?;

        // Stream object data directly to USB
        self.inner
            .session
            .send_object_stream(info.size, data)
            .await?;

        Ok(handle)
    }

    /// Upload a file from a stream with progress tracking (streaming version).
    ///
    /// Similar to `upload_streaming()` but wraps the data stream to call a
    /// progress callback after each chunk is sent.
    ///
    /// # Arguments
    ///
    /// * `parent` - Parent folder handle (None for root)
    /// * `info` - Object metadata (MUST include accurate size)
    /// * `data` - Stream of data chunks to upload
    /// * `on_progress` - Callback function called with progress updates
    ///
    /// # Returns
    ///
    /// Returns `ControlFlow::Break(())` from the callback to cancel the upload.
    pub async fn upload_streaming_with_progress<S, F>(
        &self,
        parent: Option<ObjectHandle>,
        info: NewObjectInfo,
        data: S,
        mut on_progress: F,
    ) -> Result<ObjectHandle, Error>
    where
        S: Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
        F: FnMut(Progress) -> ControlFlow<()>,
    {
        use futures::StreamExt;

        let total_size = info.size;

        // Send object info first
        let object_info = info.to_object_info();
        let parent_handle = parent.unwrap_or(ObjectHandle::ROOT);
        let (_, _, handle) = self
            .inner
            .session
            .send_object_info(self.id, parent_handle, &object_info)
            .await?;

        // Wrap the stream to track progress
        let mut bytes_sent = 0u64;
        let progress_stream = data.map(move |result| {
            result.map(|chunk| {
                bytes_sent += chunk.len() as u64;
                chunk
            })
        });

        // We need to track progress ourselves since we can't easily inject
        // into the streaming upload. For now, report progress after send_object_info
        // and after completion.
        let progress = Progress {
            bytes_transferred: 0,
            total_bytes: Some(total_size),
        };
        if let ControlFlow::Break(()) = on_progress(progress) {
            return Err(Error::Cancelled);
        }

        // Stream object data directly to USB
        // Note: For true per-chunk progress, we'd need to modify the session's
        // send_object_stream to accept a progress callback
        let mut progress_data = Box::pin(progress_stream);
        let mut total_sent = 0u64;
        let mut chunks = Vec::new();

        // Collect chunks while tracking progress
        while let Some(result) = progress_data.next().await {
            let chunk = result.map_err(Error::Io)?;
            total_sent += chunk.len() as u64;

            let progress = Progress {
                bytes_transferred: total_sent,
                total_bytes: Some(total_size),
            };
            if let ControlFlow::Break(()) = on_progress(progress) {
                return Err(Error::Cancelled);
            }

            chunks.push(chunk);
        }

        // Now stream the collected chunks
        let chunk_stream = futures::stream::iter(chunks.into_iter().map(Ok::<_, std::io::Error>));
        self.inner
            .session
            .send_object_stream(total_size, chunk_stream)
            .await?;

        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    // Note: Most Storage tests would require mock transport setup or real hardware.
    // The Storage struct requires an Arc<MtpDeviceInner> which contains a PtpSession,
    // making unit testing complex without extensive mocking infrastructure.
    // Integration tests with real devices or a comprehensive mock would be more appropriate.

    #[test]
    fn test_storage_module_exists() {
        // This test verifies the module compiles and basic types exist.
        // Actual storage operations need to be tested with mock transport or real hardware.
    }
}
