//! Integration tests for mtp-rs.
//!
//! These tests require a real MTP device (e.g., Android phone) connected via USB.
//!
//! ## Running tests
//!
//! **Read-only tests** (safe to run on any device):
//! ```sh
//! cargo test --test integration readonly -- --ignored --nocapture
//! ```
//!
//! **Destructive tests** (create/delete files on device):
//! ```sh
//! cargo test --test integration destructive -- --ignored --nocapture
//! ```
//!
//! **All tests**:
//! ```sh
//! cargo test --test integration -- --ignored --nocapture
//! ```

use serial_test::serial;

/// Read-only tests that don't modify the device.
mod readonly {
    use super::*;
    use futures::StreamExt;
    use mtp_rs::mtp::MtpDevice;
    use mtp_rs::ptp::PtpDevice;
    use std::time::Duration;

    /// Test that we can list available MTP devices.
    #[test]
    #[serial]
    fn test_list_devices() {
        let devices = MtpDevice::list_devices().unwrap();
        println!("Found {} MTP device(s)", devices.len());
        for dev in &devices {
            println!(
                "  Device: {:04x}:{:04x} at bus {} address {}",
                dev.vendor_id, dev.product_id, dev.bus, dev.address
            );
        }
    }

    /// Test connecting to a device and reading device info.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_device_connection() {
        let device = MtpDevice::open_first()
            .await
            .expect("No MTP device found. Connect an Android phone in MTP mode.");

        let info = device.device_info();
        println!("Connected to: {} {}", info.manufacturer, info.model);
        println!("  Serial: {}", info.serial_number);
        println!("  Version: {}", info.device_version);
        println!("  Vendor extension: {}", info.vendor_extension_desc);
        println!(
            "  Operations supported: {}",
            info.operations_supported.len()
        );

        assert!(!info.manufacturer.is_empty());
        assert!(!info.model.is_empty());

        device.close().await.unwrap();
    }

    /// Test listing storages on the device.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_list_storages() {
        let device = MtpDevice::open_first().await.unwrap();
        let storages = device.storages().await.unwrap();

        assert!(
            !storages.is_empty(),
            "Device should have at least one storage"
        );

        println!("Found {} storage(s):", storages.len());
        for storage in &storages {
            let info = storage.info();
            println!("  {} (ID: {:08x})", info.description, storage.id().0);
            println!(
                "    Type: {:?}, Filesystem: {:?}",
                info.storage_type, info.filesystem_type
            );
            println!(
                "    Capacity: {} bytes ({:.2} GB)",
                info.max_capacity,
                info.max_capacity as f64 / 1_000_000_000.0
            );
            println!(
                "    Free: {} bytes ({:.2} GB)",
                info.free_space_bytes,
                info.free_space_bytes as f64 / 1_000_000_000.0
            );
        }
    }

    /// Test listing files in root folder.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_list_root_folder() {
        let device = MtpDevice::open_first().await.unwrap();
        let storages = device.storages().await.unwrap();
        let storage = &storages[0];

        let objects = storage.list_objects(None).await.unwrap();

        println!("Root folder contains {} objects:", objects.len());
        for obj in &objects {
            let kind = if obj.is_folder() { "DIR " } else { "FILE" };
            println!(
                "  {} {:>12} {}",
                kind,
                if obj.is_folder() {
                    "-".to_string()
                } else {
                    format!("{}", obj.size)
                },
                obj.filename
            );
        }

        // Most Android devices have at least some folders
        assert!(objects.iter().any(|o| o.is_folder()));
    }

    /// Test recursive file listing.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_list_recursive() {
        let device = MtpDevice::open_first().await.unwrap();
        let storages = device.storages().await.unwrap();
        let storage = &storages[0];

        let objects = storage.list_objects_recursive(None).await.unwrap();

        println!("Total objects (recursive): {}", objects.len());

        let folders = objects.iter().filter(|o| o.is_folder()).count();
        let files = objects.iter().filter(|o| o.is_file()).count();
        println!("  {} folders, {} files", folders, files);

        // Show first 20 files
        println!("First 20 files:");
        for obj in objects.iter().filter(|o| o.is_file()).take(20) {
            println!("  {} ({} bytes)", obj.filename, obj.size);
        }
    }

    /// Test downloading with progress tracking.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_download_with_progress() {
        let device = MtpDevice::open_first().await.unwrap();
        let storages = device.storages().await.unwrap();
        let storage = &storages[0];

        // Find a file of reasonable size (100KB - 10MB)
        let objects = storage.list_objects_recursive(None).await.unwrap();
        let file = objects
            .iter()
            .find(|o| o.is_file() && o.size > 100_000 && o.size < 10_000_000);

        let file = match file {
            Some(f) => f,
            None => {
                println!("No suitable file found for progress test (need 100KB-10MB)");
                return;
            }
        };

        println!(
            "Downloading {} ({} bytes) with progress...",
            file.filename, file.size
        );

        let mut stream = storage.download(file.handle).await.unwrap();
        let mut last_percent = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.expect("Download error");
            if let Some(total) = chunk.total_bytes {
                let percent = (chunk.bytes_so_far * 100 / total) as u32;
                if percent >= last_percent + 10 {
                    println!("  Progress: {}%", percent);
                    last_percent = percent;
                }
            }
        }

        println!("Download complete");
    }

    /// Test custom timeout configuration.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_custom_timeout() {
        let device = MtpDevice::builder()
            .timeout(Duration::from_secs(60))
            .open_first()
            .await
            .expect("Failed to open device with custom timeout");

        println!(
            "Opened device with 60s timeout: {}",
            device.device_info().model
        );

        device.close().await.unwrap();
    }

    /// Test low-level PtpDevice API.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_ptp_device() {
        let device = PtpDevice::open_first().await.expect("No PTP device found");

        // Get device info without session
        let info = device.get_device_info().await.unwrap();
        println!("PTP Device: {} {}", info.manufacturer, info.model);

        // Open session
        let session = device.open_session().await.unwrap();
        println!("Session opened");

        // Get storage IDs through session
        let storage_ids = session.get_storage_ids().await.unwrap();
        println!("Storage IDs: {:?}", storage_ids);

        session.close().await.unwrap();
        println!("Session closed");
    }

    /// Test refreshing storage info.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_refresh_storage() {
        let device = MtpDevice::open_first().await.unwrap();
        let mut storages = device.storages().await.unwrap();
        let storage = &mut storages[0];

        let initial_free = storage.info().free_space_bytes;
        println!("Initial free space: {} bytes", initial_free);

        // Refresh
        storage.refresh().await.unwrap();

        let refreshed_free = storage.info().free_space_bytes;
        println!("After refresh: {} bytes", refreshed_free);

        // Values should be similar (might differ slightly due to system activity)
    }
}

/// Destructive tests that create/modify/delete files on the device.
///
/// **Warning**: These tests write to the device. Use with caution.
mod destructive {
    use super::*;
    use bytes::Bytes;
    use mtp_rs::mtp::{MtpDevice, NewObjectInfo};
    use mtp_rs::Error;

    /// Test uploading, downloading, and deleting a file.
    #[tokio::test]
    #[ignore] // Requires real MTP device - WRITES TO DEVICE
    #[serial]
    async fn test_upload_download_delete() {
        let device = MtpDevice::open_first().await.unwrap();
        let storages = device.storages().await.unwrap();
        let storage = &storages[0];

        // Find Download folder (Android doesn't allow creating files in root)
        let root_objects = storage.list_objects(None).await.unwrap();
        let download_folder = root_objects
            .iter()
            .find(|o| o.filename == "Download")
            .expect("Download folder not found");
        println!("Using Download folder (handle: {:?})", download_folder.handle);

        // Create test content
        let test_content = format!(
            "Test file created by mtp-rs integration test at {:?}",
            std::time::SystemTime::now()
        );
        let content_bytes = test_content.as_bytes();

        println!("Uploading test file ({} bytes)...", content_bytes.len());

        // Upload to Download folder
        let info = NewObjectInfo::file("mtp-rs-test.txt", content_bytes.len() as u64);
        let data_stream = futures::stream::iter(vec![Ok::<_, std::io::Error>(Bytes::from(
            content_bytes.to_vec(),
        ))]);

        let handle = storage
            .upload(Some(download_folder.handle), info, Box::pin(data_stream))
            .await
            .expect("Upload failed");

        println!("Uploaded with handle: {:?}", handle);

        // Verify object info
        let obj_info = storage.get_object_info(handle).await.unwrap();
        assert_eq!(obj_info.filename, "mtp-rs-test.txt");
        assert_eq!(obj_info.size, content_bytes.len() as u64);
        println!("Object info verified");

        // Download
        println!("Downloading...");
        let download_stream = storage.download(handle).await.unwrap();
        let downloaded = download_stream.collect().await.unwrap();

        assert_eq!(downloaded, content_bytes, "Downloaded content doesn't match");
        println!("Download verified");

        // Delete
        println!("Deleting...");
        storage.delete(handle).await.expect("Delete failed");

        // Verify deleted
        let result = storage.get_object_info(handle).await;
        assert!(
            matches!(
                result,
                Err(Error::Protocol {
                    code: mtp_rs::ptp::ResponseCode::InvalidObjectHandle,
                    ..
                })
            ),
            "Object should be deleted"
        );
        println!("Delete verified");

        println!("Upload/download/delete test PASSED");
    }

    /// Test creating and deleting a folder.
    #[tokio::test]
    #[ignore] // Requires real MTP device - WRITES TO DEVICE
    #[serial]
    async fn test_create_delete_folder() {
        let device = MtpDevice::open_first().await.unwrap();
        let storages = device.storages().await.unwrap();
        let storage = &storages[0];

        // Find Download folder (Android doesn't allow creating folders in root)
        let root_objects = storage.list_objects(None).await.unwrap();
        let download_folder = root_objects
            .iter()
            .find(|o| o.filename == "Download")
            .expect("Download folder not found");
        println!("Using Download folder (handle: {:?})", download_folder.handle);

        let folder_name = format!("mtp-rs-test-{}", std::process::id());
        println!("Creating folder: {}", folder_name);

        // Create folder inside Download
        let handle = storage
            .create_folder(Some(download_folder.handle), &folder_name)
            .await
            .expect("Create folder failed");

        println!("Created folder with handle: {:?}", handle);

        // Verify it exists
        let info = storage.get_object_info(handle).await.unwrap();
        assert!(info.is_folder());
        assert_eq!(info.filename, folder_name);

        // Delete it
        println!("Deleting folder...");
        storage.delete(handle).await.expect("Delete folder failed");

        println!("Folder create/delete test PASSED");
    }
}
