//! Virtual MTP device transport for testing.
//!
//! This module provides a [`VirtualTransport`] that implements the [`Transport`] trait
//! using a local filesystem directory as its backing store. It speaks the full MTP/PTP
//! binary protocol, so the existing `PtpSession`, `MtpDevice`, and `Storage` types
//! work unchanged.
//!
//! Use this to test MTP client code without real USB hardware.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::path::PathBuf;
//! use std::time::Duration;
//! use mtp_rs::MtpDevice;
//! use mtp_rs::transport::virtual_device::config::{VirtualDeviceConfig, VirtualStorageConfig};
//!
//! # async fn example() -> Result<(), mtp_rs::Error> {
//! let device = MtpDevice::builder()
//!     .open_virtual(VirtualDeviceConfig {
//!         manufacturer: "Google".into(),
//!         model: "Virtual Pixel 9".into(),
//!         serial: "virtual-001".into(),
//!         storages: vec![VirtualStorageConfig {
//!             description: "Internal Storage".into(),
//!             capacity: 64 * 1024 * 1024 * 1024,
//!             backing_dir: PathBuf::from("/tmp/mtp-test"),
//!             read_only: false,
//!         }],
//!         supports_rename: true,
//!         event_poll_interval: Duration::from_millis(50),
//!     })
//!     .await?;
//!
//! // Use the device exactly like a real one
//! for storage in device.storages().await? {
//!     for obj in storage.list_objects(None).await? {
//!         println!("{}", obj.filename);
//!     }
//! }
//! # Ok(())
//! # }
//! ```

mod builders;
pub mod config;
mod handlers;
pub mod registry;
mod state;

use crate::ptp::{unpack_u16, unpack_u32};
use crate::transport::Transport;
use async_trait::async_trait;
use config::VirtualDeviceConfig;
use state::{PendingCommand, VirtualDeviceState};
use std::sync::Mutex;
use std::time::Duration;

/// A transport that speaks MTP/PTP binary protocol against a local filesystem.
///
/// Created via `MtpDeviceBuilder::open_virtual()` or directly for lower-level use.
///
/// Internally, incoming `send_bulk` calls are parsed as MTP command/data containers.
/// The virtual device processes each operation (list files, read, write, delete, etc.)
/// against the configured backing directories and queues binary response containers
/// for the next `receive_bulk` call.
pub struct VirtualTransport {
    state: Mutex<VirtualDeviceState>,
    /// How long `receive_interrupt` waits when no events are pending.
    event_poll_interval: Duration,
}

impl VirtualTransport {
    /// Create a new virtual transport from a device configuration.
    ///
    /// The backing directories in each storage config should already exist.
    #[must_use]
    pub fn new(config: VirtualDeviceConfig) -> Self {
        let event_poll_interval = config.event_poll_interval;
        Self {
            state: Mutex::new(VirtualDeviceState::new(config)),
            event_poll_interval,
        }
    }
}

/// Container type constants.
const CONTAINER_TYPE_COMMAND: u16 = 1;
const CONTAINER_TYPE_DATA: u16 = 2;

#[async_trait]
impl Transport for VirtualTransport {
    async fn send_bulk(&self, data: &[u8]) -> Result<(), crate::Error> {
        if data.len() < 12 {
            return Err(crate::Error::invalid_data("container too small"));
        }

        let _length = unpack_u32(&data[0..4])?;
        let container_type = unpack_u16(&data[4..6])?;
        let code = unpack_u16(&data[6..8])?;
        let tx_id = unpack_u32(&data[8..12])?;

        let mut state = self.state.lock().unwrap();

        match container_type {
            CONTAINER_TYPE_COMMAND => {
                // Parse parameters (each u32, after the 12-byte header)
                let param_bytes = data.len() - 12;
                let param_count = param_bytes / 4;
                let mut params = Vec::with_capacity(param_count);
                for i in 0..param_count {
                    let offset = 12 + i * 4;
                    params.push(unpack_u32(&data[offset..])?);
                }

                // Check if this operation expects a data phase from the host.
                // If so, don't dispatch yet -- store the command and wait for data.
                let op = crate::ptp::OperationCode::from(code);
                if matches!(
                    op,
                    crate::ptp::OperationCode::SendObjectInfo
                        | crate::ptp::OperationCode::SendObject
                        | crate::ptp::OperationCode::SetObjectPropValue
                ) {
                    state.pending_command = Some(PendingCommand {
                        code,
                        tx_id,
                        params,
                    });
                } else {
                    handlers::dispatch(&mut state, code, tx_id, &params, None);
                }
            }
            CONTAINER_TYPE_DATA => {
                // This is the data phase for a previous command.
                match state.pending_command.take() {
                    Some(pending) => {
                        let payload = &data[12..]; // Skip data container header
                        handlers::dispatch(
                            &mut state,
                            pending.code,
                            pending.tx_id,
                            &pending.params,
                            Some(payload),
                        );
                    }
                    None => {
                        return Err(crate::Error::invalid_data(
                            "received data container without pending command",
                        ));
                    }
                }
            }
            _ => {
                return Err(crate::Error::invalid_data(format!(
                    "unexpected container type: {}",
                    container_type
                )));
            }
        }

        Ok(())
    }

    async fn receive_bulk(&self, _max_size: usize) -> Result<Vec<u8>, crate::Error> {
        let mut state = self.state.lock().unwrap();
        match state.response_queue.pop_front() {
            Some(data) => Ok(data),
            None => Err(crate::Error::invalid_data("no response available")),
        }
    }

    async fn receive_interrupt(&self) -> Result<Vec<u8>, crate::Error> {
        // Check for events (drop lock before any await)
        {
            let mut state = self.state.lock().unwrap();
            if let Some(event) = state.event_queue.pop_front() {
                return Ok(event);
            }
        }
        // No events — wait, then return Timeout
        futures_timer::Delay::new(self.event_poll_interval).await;
        Err(crate::Error::Timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::config::{VirtualDeviceConfig, VirtualStorageConfig};
    use crate::mtp::MtpDevice;
    use crate::ptp::ObjectFormatCode;
    use std::time::Duration;

    fn test_config(dir: &std::path::Path) -> VirtualDeviceConfig {
        VirtualDeviceConfig {
            manufacturer: "TestCorp".into(),
            model: "Virtual Phone".into(),
            serial: "test-001".into(),
            storages: vec![VirtualStorageConfig {
                description: "Internal Storage".into(),
                capacity: 1024 * 1024 * 1024, // 1 GB
                backing_dir: dir.to_path_buf(),
                read_only: false,
            }],
            supports_rename: true,
            event_poll_interval: Duration::ZERO,
        }
    }

    fn test_config_readonly(dir: &std::path::Path) -> VirtualDeviceConfig {
        VirtualDeviceConfig {
            manufacturer: "TestCorp".into(),
            model: "Virtual Phone".into(),
            serial: "test-ro".into(),
            storages: vec![VirtualStorageConfig {
                description: "Read-only Storage".into(),
                capacity: 1024 * 1024 * 1024,
                backing_dir: dir.to_path_buf(),
                read_only: true,
            }],
            supports_rename: true,
            event_poll_interval: Duration::ZERO,
        }
    }

    fn test_config_multi(dirs: &[&std::path::Path]) -> VirtualDeviceConfig {
        VirtualDeviceConfig {
            manufacturer: "TestCorp".into(),
            model: "Virtual Phone".into(),
            serial: "test-multi".into(),
            storages: dirs
                .iter()
                .enumerate()
                .map(|(i, d)| VirtualStorageConfig {
                    description: format!("Storage {}", i + 1),
                    capacity: 1024 * 1024 * 1024,
                    backing_dir: d.to_path_buf(),
                    read_only: false,
                })
                .collect(),
            supports_rename: true,
            event_poll_interval: Duration::ZERO,
        }
    }

    /// Helper to convert `&[u8]` to a `Stream<Item = Result<Bytes, io::Error>>`.
    fn bytes_stream(
        data: &[u8],
    ) -> impl futures::Stream<Item = Result<bytes::Bytes, std::io::Error>> + Unpin {
        futures::stream::once(futures::future::ok(bytes::Bytes::copy_from_slice(data)))
    }

    #[tokio::test]
    async fn open_virtual_and_list_storages() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        assert_eq!(storages.len(), 1);
        assert_eq!(storages[0].info().description, "Internal Storage");
        assert!(storages[0].info().max_capacity > 0);
    }

    #[tokio::test]
    async fn device_info_matches_config() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let info = device.device_info();

        assert_eq!(info.manufacturer, "TestCorp");
        assert_eq!(info.model, "Virtual Phone");
        assert_eq!(info.serial_number, "test-001");
        assert!(device.supports_rename());
    }

    #[tokio::test]
    async fn list_objects_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();
        let objects = storages[0].list_objects(None).await.unwrap();

        assert!(objects.is_empty());
    }

    #[tokio::test]
    async fn list_objects_with_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
        std::fs::write(dir.path().join("photo.jpg"), "fake jpeg data").unwrap();
        std::fs::create_dir(dir.path().join("Documents")).unwrap();

        let config = test_config(dir.path());
        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();
        let items = storages[0].list_objects(None).await.unwrap();

        assert_eq!(items.len(), 3);
        let names: Vec<&str> = items.iter().map(|i| i.filename.as_str()).collect();
        assert!(names.contains(&"hello.txt"));
        assert!(names.contains(&"photo.jpg"));
        assert!(names.contains(&"Documents"));

        // Verify folder detection
        let docs = items.iter().find(|i| i.filename == "Documents").unwrap();
        assert!(docs.is_folder());
        assert_eq!(docs.format, ObjectFormatCode::Association);

        // Verify file metadata
        let txt = items.iter().find(|i| i.filename == "hello.txt").unwrap();
        assert!(txt.is_file());
        assert_eq!(txt.size, 11); // "hello world" = 11 bytes
    }

    #[tokio::test]
    async fn download_file() {
        let dir = tempfile::tempdir().unwrap();
        let content = b"test file content for download";
        std::fs::write(dir.path().join("test.txt"), content).unwrap();

        let config = test_config(dir.path());
        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();
        let items = storages[0].list_objects(None).await.unwrap();
        let obj = &items[0];

        let data = storages[0].download(obj.handle).await.unwrap();
        assert_eq!(data.as_slice(), content);
    }

    #[tokio::test]
    async fn upload_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        let info = crate::mtp::NewObjectInfo::file("uploaded.txt", 12);
        let handle = storages[0]
            .upload(None, info, bytes_stream(b"hello upload"))
            .await
            .unwrap();

        // Verify file exists on disk
        let path = dir.path().join("uploaded.txt");
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello upload");

        // Verify we can download it back
        let data = storages[0].download(handle).await.unwrap();
        assert_eq!(data.as_slice(), b"hello upload");
    }

    #[tokio::test]
    async fn upload_to_subfolder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Music")).unwrap();

        let config = test_config(dir.path());
        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        // List root to get the Music folder handle
        let items = storages[0].list_objects(None).await.unwrap();
        let music = items.iter().find(|i| i.filename == "Music").unwrap();
        assert!(music.is_folder());

        // Upload a file into Music
        let info = crate::mtp::NewObjectInfo::file("song.mp3", 5);
        storages[0]
            .upload(Some(music.handle), info, bytes_stream(b"audio"))
            .await
            .unwrap();

        assert!(dir.path().join("Music/song.mp3").exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("Music/song.mp3")).unwrap(),
            "audio"
        );
    }

    #[tokio::test]
    async fn delete_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("doomed.txt"), "goodbye").unwrap();

        let config = test_config(dir.path());
        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        let items = storages[0].list_objects(None).await.unwrap();
        let obj = &items[0];

        storages[0].delete(obj.handle).await.unwrap();
        assert!(!dir.path().join("doomed.txt").exists());
    }

    #[tokio::test]
    async fn create_folder() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        storages[0].create_folder(None, "NewFolder").await.unwrap();

        assert!(dir.path().join("NewFolder").is_dir());
    }

    #[tokio::test]
    async fn rename_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("old_name.txt"), "content").unwrap();

        let config = test_config(dir.path());
        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        let items = storages[0].list_objects(None).await.unwrap();
        let obj = &items[0];

        storages[0]
            .rename(obj.handle, "new_name.txt")
            .await
            .unwrap();

        assert!(!dir.path().join("old_name.txt").exists());
        assert!(dir.path().join("new_name.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("new_name.txt")).unwrap(),
            "content"
        );
    }

    #[tokio::test]
    async fn nested_directories() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b/c")).unwrap();
        std::fs::write(dir.path().join("a/b/c/deep.txt"), "deep").unwrap();
        std::fs::write(dir.path().join("a/top.txt"), "top").unwrap();

        let config = test_config(dir.path());
        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        // List root
        let root_items = storages[0].list_objects(None).await.unwrap();
        assert_eq!(root_items.len(), 1); // Only "a"
        assert_eq!(root_items[0].filename, "a");
        assert!(root_items[0].is_folder());
    }

    #[tokio::test]
    async fn read_only_storage_rejects_writes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("existing.txt"), "data").unwrap();

        let config = test_config_readonly(dir.path());
        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        // Verify read-only access capability is reported
        assert_ne!(
            storages[0].info().access_capability,
            crate::ptp::AccessCapability::ReadWrite
        );

        // Upload should fail
        let info = crate::mtp::NewObjectInfo::file("new.txt", 4);
        let result = storages[0].upload(None, info, bytes_stream(b"data")).await;
        assert!(result.is_err());

        // Delete should fail
        let items = storages[0].list_objects(None).await.unwrap();
        let result = storages[0].delete(items[0].handle).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn multiple_storages() {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();
        std::fs::write(dir1.path().join("file1.txt"), "storage1").unwrap();
        std::fs::write(dir2.path().join("file2.txt"), "storage2").unwrap();

        let config = test_config_multi(&[dir1.path(), dir2.path()]);
        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        assert_eq!(storages.len(), 2);

        let items1 = storages[0].list_objects(None).await.unwrap();
        assert_eq!(items1.len(), 1);
        assert_eq!(items1[0].filename, "file1.txt");

        let items2 = storages[1].list_objects(None).await.unwrap();
        assert_eq!(items2.len(), 1);
        assert_eq!(items2[0].filename, "file2.txt");
    }

    #[tokio::test]
    async fn free_space_reflects_content() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();
        let free_before = storages[0].info().free_space_bytes;

        // Upload a file
        let info = crate::mtp::NewObjectInfo::file("big.bin", 1000);
        let data = vec![0u8; 1000];
        storages[0]
            .upload(None, info, bytes_stream(&data))
            .await
            .unwrap();

        // Re-fetch storage info
        let storages2 = device.storages().await.unwrap();
        let free_after = storages2[0].info().free_space_bytes;

        assert!(free_after < free_before);
        assert_eq!(free_before - free_after, 1000);
    }

    #[tokio::test]
    async fn events_generated_on_upload() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        let info = crate::mtp::NewObjectInfo::file("event_test.txt", 5);
        storages[0]
            .upload(None, info, bytes_stream(b"hello"))
            .await
            .unwrap();

        // Events should be available (ObjectAdded + StorageInfoChanged)
        use tokio::time::{timeout, Duration};
        let event = timeout(Duration::from_millis(100), device.next_event()).await;
        assert!(event.is_ok());
    }

    #[tokio::test]
    async fn events_generated_on_delete() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("to_delete.txt"), "bye").unwrap();

        let config = test_config(dir.path());
        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        let items = storages[0].list_objects(None).await.unwrap();
        storages[0].delete(items[0].handle).await.unwrap();

        // Should have ObjectRemoved + StorageInfoChanged events
        use tokio::time::{timeout, Duration};
        let event = timeout(Duration::from_millis(100), device.next_event()).await;
        assert!(event.is_ok());
    }

    #[tokio::test]
    async fn no_events_returns_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let device = MtpDevice::builder().open_virtual(config).await.unwrap();

        // No operations performed, so no events
        let result = device.next_event().await;
        assert!(matches!(result, Err(crate::Error::Timeout)));
    }

    #[tokio::test]
    async fn copy_object() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("original.txt"), "copy me").unwrap();
        std::fs::create_dir(dir.path().join("dest")).unwrap();

        let config = test_config(dir.path());
        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        let items = storages[0].list_objects(None).await.unwrap();
        let original = items.iter().find(|i| i.filename == "original.txt").unwrap();
        let dest = items.iter().find(|i| i.filename == "dest").unwrap();

        storages[0]
            .copy_object(original.handle, dest.handle, None)
            .await
            .unwrap();

        // Both should exist
        assert!(dir.path().join("original.txt").exists());
        assert!(dir.path().join("dest/original.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("dest/original.txt")).unwrap(),
            "copy me"
        );
    }

    #[tokio::test]
    async fn path_traversal_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        // Try to upload a file with ".." in the name
        let info = crate::mtp::NewObjectInfo::file("../escape.txt", 6);
        let result = storages[0]
            .upload(None, info, bytes_stream(b"escape"))
            .await;
        assert!(result.is_err(), "path traversal upload should be rejected");

        // Verify the file was NOT created outside the backing dir
        assert!(!dir.path().parent().unwrap().join("escape.txt").exists());
    }

    #[tokio::test]
    async fn move_object() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("moveme.txt"), "move me").unwrap();
        std::fs::create_dir(dir.path().join("target")).unwrap();

        let config = test_config(dir.path());
        let device = MtpDevice::builder().open_virtual(config).await.unwrap();
        let storages = device.storages().await.unwrap();

        let items = storages[0].list_objects(None).await.unwrap();
        let moveme = items.iter().find(|i| i.filename == "moveme.txt").unwrap();
        let target = items.iter().find(|i| i.filename == "target").unwrap();

        storages[0]
            .move_object(moveme.handle, target.handle, None)
            .await
            .unwrap();

        assert!(!dir.path().join("moveme.txt").exists());
        assert!(dir.path().join("target/moveme.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("target/moveme.txt")).unwrap(),
            "move me"
        );
    }
}
