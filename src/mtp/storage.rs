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
    pub async fn list_objects(
        &self,
        parent: Option<ObjectHandle>,
    ) -> Result<Vec<ObjectInfo>, Error> {
        let handles = self
            .inner
            .session
            .get_object_handles(
                self.id,
                None,   // All formats
                parent, // None means root (handle 0)
            )
            .await?;

        let mut objects = Vec::with_capacity(handles.len());
        for handle in handles {
            let mut info = self.inner.session.get_object_info(handle).await?;
            info.handle = handle;
            objects.push(info);
        }
        Ok(objects)
    }

    /// List objects recursively.
    pub async fn list_objects_recursive(
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
