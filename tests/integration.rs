//! Integration tests for mtp-rs.
//!
//! These tests require a real MTP device (e.g., Android phone) connected via USB.
//!
//! # ============================================================================
//! # WARNING: SERIAL EXECUTION IS MANDATORY
//! # ============================================================================
//! #
//! # MTP devices can only handle ONE operation at a time. Running tests in
//! # parallel WILL cause failures, timeouts, and flaky behavior.
//! #
//! # YOU MUST USE: --test-threads=1
//! #
//! # If you forget this flag, the tests will detect parallel execution and
//! # panic with a clear error message.
//! #
//! # ============================================================================
//!
//! ## Running tests
//!
//! **Read-only tests** (safe to run on any device):
//! ```sh
//! cargo test --test integration readonly -- --ignored --nocapture --test-threads=1
//! ```
//!
//! **Destructive tests** (create/delete files on device):
//! ```sh
//! cargo test --test integration destructive -- --ignored --nocapture --test-threads=1
//! ```
//!
//! **All tests** (excluding slow tests):
//! ```sh
//! cargo test --test integration -- --ignored --nocapture --test-threads=1 --skip slow
//! ```
//!
//! **All tests including slow**:
//! ```sh
//! cargo test --test integration -- --ignored --nocapture --test-threads=1
//! ```
//!
//! ## Slow tests
//!
//! Tests prefixed with `slow_` can take several minutes (e.g., recursive listing on a device
//! with thousands of files). They are skipped by default due to `#[ignore]` and `--skip slow`
//! is recommended.

use serial_test::serial;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

/// Global test start time - initialized lazily on first use
static TEST_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Counter for detecting parallel test execution
static RUNNING_TESTS: AtomicU32 = AtomicU32::new(0);

/// Get the elapsed time since tests started, formatted as [HH:MM:SS.mmm]
fn elapsed_timestamp() -> String {
    let start = TEST_START.get_or_init(Instant::now);
    let elapsed = start.elapsed();
    let total_secs = elapsed.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    let millis = elapsed.subsec_millis();
    format!("[{:02}:{:02}:{:02}.{:03}]", hours, minutes, seconds, millis)
}

/// Timestamped logging macro - prints messages with elapsed time prefix
macro_rules! tlog {
    ($($arg:tt)*) => {{
        println!("{} {}", $crate::elapsed_timestamp(), format_args!($($arg)*));
    }};
}

/// Guard that tracks test execution and detects parallel runs.
/// When created, increments the running test counter and checks for parallelism.
/// When dropped, decrements the counter.
struct TestGuard {
    test_name: &'static str,
}

impl TestGuard {
    fn new(test_name: &'static str) -> Self {
        // Initialize the start time on first test
        let _ = TEST_START.get_or_init(Instant::now);

        let count = RUNNING_TESTS.fetch_add(1, Ordering::SeqCst);

        tlog!("=== Starting test: {} ===", test_name);

        if count > 0 {
            tlog!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
            tlog!("!! PARALLEL EXECUTION DETECTED! {} tests running simultaneously", count + 1);
            tlog!("!! MTP tests MUST run with --test-threads=1");
            tlog!("!! Run with: cargo test --test integration -- --test-threads=1");
            tlog!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
            panic!(
                "PARALLEL TEST EXECUTION DETECTED!\n\
                 MTP devices can only handle one operation at a time.\n\
                 You MUST run tests with --test-threads=1\n\
                 Example: cargo test --test integration -- --ignored --nocapture --test-threads=1"
            );
        }

        TestGuard { test_name }
    }
}

impl Drop for TestGuard {
    fn drop(&mut self) {
        RUNNING_TESTS.fetch_sub(1, Ordering::SeqCst);
        tlog!("=== Finished test: {} ===", self.test_name);
    }
}

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
        let _guard = TestGuard::new("test_list_devices");

        tlog!("Listing MTP devices...");
        let devices = MtpDevice::list_devices().unwrap();
        tlog!("Found {} MTP device(s)", devices.len());
        for dev in &devices {
            tlog!(
                "  Device: {:04x}:{:04x} at bus {} address {}",
                dev.vendor_id, dev.product_id, dev.bus, dev.address
            );
        }
        tlog!("Device listing complete");
    }

    /// Test connecting to a device and reading device info.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_device_connection() {
        let _guard = TestGuard::new("test_device_connection");

        tlog!("Opening first MTP device...");
        let device = MtpDevice::open_first()
            .await
            .expect("No MTP device found. Connect an Android phone in MTP mode.");

        let info = device.device_info();
        tlog!("Device opened: {} {}", info.manufacturer, info.model);
        tlog!("  Serial: {}", info.serial_number);
        tlog!("  Version: {}", info.device_version);
        tlog!("  Vendor extension: {}", info.vendor_extension_desc);
        tlog!(
            "  Operations supported: {}",
            info.operations_supported.len()
        );

        assert!(!info.manufacturer.is_empty());
        assert!(!info.model.is_empty());

        tlog!("Closing device...");
        device.close().await.unwrap();
        tlog!("Device closed");
    }

    /// Test listing storages on the device.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_list_storages() {
        let _guard = TestGuard::new("test_list_storages");

        tlog!("Opening device...");
        let device = MtpDevice::open_first().await.unwrap();
        tlog!("Device opened: {}", device.device_info().model);

        tlog!("Getting storages...");
        let storages = device.storages().await.unwrap();
        tlog!("Found {} storage(s)", storages.len());

        assert!(
            !storages.is_empty(),
            "Device should have at least one storage"
        );

        for storage in &storages {
            let info = storage.info();
            tlog!("  {} (ID: {:08x})", info.description, storage.id().0);
            tlog!(
                "    Type: {:?}, Filesystem: {:?}",
                info.storage_type, info.filesystem_type
            );
            tlog!(
                "    Capacity: {} bytes ({:.2} GB)",
                info.max_capacity,
                info.max_capacity as f64 / 1_000_000_000.0
            );
            tlog!(
                "    Free: {} bytes ({:.2} GB)",
                info.free_space_bytes,
                info.free_space_bytes as f64 / 1_000_000_000.0
            );
        }
        tlog!("Storage listing complete");
    }

    /// Test listing files in root folder.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_list_root_folder() {
        let _guard = TestGuard::new("test_list_root_folder");

        tlog!("Opening device...");
        let device = MtpDevice::open_first().await.unwrap();
        tlog!("Device opened: {}", device.device_info().model);

        tlog!("Getting storages...");
        let storages = device.storages().await.unwrap();
        let storage = &storages[0];
        tlog!("Using storage: {}", storage.info().description);

        tlog!("Listing root folder objects...");
        let objects = storage.list_objects(None).await.unwrap();
        tlog!("Root folder contains {} objects", objects.len());

        for obj in &objects {
            let kind = if obj.is_folder() { "DIR " } else { "FILE" };
            tlog!(
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
        tlog!("Root folder listing complete");
    }

    /// Test recursive file listing.
    ///
    /// **SLOW TEST**: This test can take 5-10+ minutes on devices with many files.
    /// It lists ALL objects on the device recursively.
    ///
    /// This test is skipped by default. Run explicitly with:
    /// `cargo test slow_test_list_recursive -- --ignored --nocapture --test-threads=1`
    #[tokio::test]
    #[ignore] // Requires real MTP device AND is very slow - double-ignored effectively
    #[serial]
    async fn slow_test_list_recursive() {
        let _guard = TestGuard::new("slow_test_list_recursive");

        // Additional skip check - only run if explicitly requested
        if std::env::var("MTP_RUN_SLOW_TESTS").is_err() {
            tlog!("SKIPPING: slow_test_list_recursive");
            tlog!("This test can take 5-10+ minutes. To run it, set MTP_RUN_SLOW_TESTS=1");
            tlog!("Example: MTP_RUN_SLOW_TESTS=1 cargo test slow_test_list_recursive -- --ignored --nocapture --test-threads=1");
            return;
        }

        tlog!("Opening device...");
        let device = MtpDevice::open_first().await.unwrap();
        tlog!("Device opened: {}", device.device_info().model);

        tlog!("Getting storages...");
        let storages = device.storages().await.unwrap();
        let storage = &storages[0];
        tlog!("Using storage: {}", storage.info().description);

        tlog!("Starting recursive listing (this may take several minutes)...");
        let objects = storage.list_objects_recursive(None).await.unwrap();
        tlog!("Recursive listing complete");

        tlog!("Total objects (recursive): {}", objects.len());

        let folders = objects.iter().filter(|o| o.is_folder()).count();
        let files = objects.iter().filter(|o| o.is_file()).count();
        tlog!("  {} folders, {} files", folders, files);

        // Show first 20 files
        tlog!("First 20 files:");
        for obj in objects.iter().filter(|o| o.is_file()).take(20) {
            tlog!("  {} ({} bytes)", obj.filename, obj.size);
        }
    }

    /// Test downloading with progress tracking.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_download_with_progress() {
        let _guard = TestGuard::new("test_download_with_progress");

        tlog!("Opening device...");
        let device = MtpDevice::open_first().await.unwrap();
        tlog!("Device opened: {}", device.device_info().model);

        tlog!("Getting storages...");
        let storages = device.storages().await.unwrap();
        let storage = &storages[0];
        tlog!("Using storage: {}", storage.info().description);

        // Find a file of reasonable size (100KB - 10MB)
        tlog!("Searching for suitable file (100KB-10MB) via recursive listing...");
        let objects = storage.list_objects_recursive(None).await.unwrap();
        tlog!("Found {} total objects", objects.len());

        let file = objects
            .iter()
            .find(|o| o.is_file() && o.size > 100_000 && o.size < 10_000_000);

        let file = match file {
            Some(f) => f,
            None => {
                tlog!("No suitable file found for progress test (need 100KB-10MB)");
                return;
            }
        };

        tlog!(
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
                    tlog!("  Progress: {}%", percent);
                    last_percent = percent;
                }
            }
        }

        tlog!("Download complete");
    }

    /// Test custom timeout configuration.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_custom_timeout() {
        let _guard = TestGuard::new("test_custom_timeout");

        tlog!("Opening device with custom 60s timeout...");
        let device = MtpDevice::builder()
            .timeout(Duration::from_secs(60))
            .open_first()
            .await
            .expect("Failed to open device with custom timeout");

        tlog!(
            "Device opened with 60s timeout: {}",
            device.device_info().model
        );

        tlog!("Closing device...");
        device.close().await.unwrap();
        tlog!("Device closed");
    }

    /// Test low-level PtpDevice API.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_ptp_device() {
        let _guard = TestGuard::new("test_ptp_device");

        tlog!("Opening first PTP device...");
        let device = PtpDevice::open_first().await.expect("No PTP device found");

        // Get device info without session
        tlog!("Getting device info...");
        let info = device.get_device_info().await.unwrap();
        tlog!("PTP Device: {} {}", info.manufacturer, info.model);

        // Open session
        tlog!("Opening PTP session...");
        let session = device.open_session().await.unwrap();
        tlog!("Session opened");

        // Get storage IDs through session
        tlog!("Getting storage IDs...");
        let storage_ids = session.get_storage_ids().await.unwrap();
        tlog!("Storage IDs: {:?}", storage_ids);

        tlog!("Closing session...");
        session.close().await.unwrap();
        tlog!("Session closed");
    }

    /// Test refreshing storage info.
    #[tokio::test]
    #[ignore] // Requires real MTP device
    #[serial]
    async fn test_refresh_storage() {
        let _guard = TestGuard::new("test_refresh_storage");

        tlog!("Opening device...");
        let device = MtpDevice::open_first().await.unwrap();
        tlog!("Device opened: {}", device.device_info().model);

        tlog!("Getting storages...");
        let mut storages = device.storages().await.unwrap();
        let storage = &mut storages[0];
        tlog!("Using storage: {}", storage.info().description);

        let initial_free = storage.info().free_space_bytes;
        tlog!("Initial free space: {} bytes", initial_free);

        // Refresh
        tlog!("Refreshing storage info...");
        storage.refresh().await.unwrap();

        let refreshed_free = storage.info().free_space_bytes;
        tlog!("After refresh: {} bytes", refreshed_free);

        // Values should be similar (might differ slightly due to system activity)
        tlog!("Storage refresh complete");
    }
}

// NOTE: Camera control tests are disabled until the PtpSession methods for
// device properties (get_device_prop_desc, get_device_prop_value_typed,
// reset_device_prop_value, initiate_capture) are implemented.
//
// These tests are designed for digital cameras that support PTP device properties
// and capture operations. Most Android MTP devices do not support these features.
//
// To re-enable: uncomment the module below and implement the missing PtpSession methods.

/*
/// Camera control tests for PTP devices with camera functionality.
///
/// These tests work with digital cameras and devices that support
/// device properties and capture operations.
mod camera {
    use super::*;
    use mtp_rs::ptp::{
        DevicePropertyCode, ObjectFormatCode, PropertyDataType, PropertyValue, PtpDevice, StorageId,
    };

    // ... tests commented out until PtpSession device property methods are implemented ...
}
*/

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
        let _guard = TestGuard::new("test_upload_download_delete");

        tlog!("Opening device...");
        let device = MtpDevice::open_first().await.unwrap();
        tlog!("Device opened: {}", device.device_info().model);

        tlog!("Getting storages...");
        let storages = device.storages().await.unwrap();
        let storage = &storages[0];
        tlog!("Using storage: {}", storage.info().description);

        // Find Download folder (Android doesn't allow creating files in root)
        tlog!("Listing root folder to find Download...");
        let root_objects = storage.list_objects(None).await.unwrap();
        let download_folder = root_objects
            .iter()
            .find(|o| o.filename == "Download")
            .expect("Download folder not found");
        tlog!(
            "Using Download folder (handle: {:?})",
            download_folder.handle
        );

        // Create test content
        let test_content = format!(
            "Test file created by mtp-rs integration test at {:?}",
            std::time::SystemTime::now()
        );
        let content_bytes = test_content.as_bytes();

        tlog!("Uploading test file ({} bytes)...", content_bytes.len());

        // Upload to Download folder
        let info = NewObjectInfo::file("mtp-rs-test.txt", content_bytes.len() as u64);
        let data_stream = futures::stream::iter(vec![Ok::<_, std::io::Error>(Bytes::from(
            content_bytes.to_vec(),
        ))]);

        let handle = storage
            .upload(Some(download_folder.handle), info, Box::pin(data_stream))
            .await
            .expect("Upload failed");

        tlog!("Upload complete, handle: {:?}", handle);

        // Verify object info
        tlog!("Verifying uploaded object info...");
        let obj_info = storage.get_object_info(handle).await.unwrap();
        assert_eq!(obj_info.filename, "mtp-rs-test.txt");
        assert_eq!(obj_info.size, content_bytes.len() as u64);
        tlog!("Object info verified: {} ({} bytes)", obj_info.filename, obj_info.size);

        // Download
        tlog!("Downloading file...");
        let download_stream = storage.download(handle).await.unwrap();
        let downloaded = download_stream.collect().await.unwrap();
        tlog!("Download complete, {} bytes received", downloaded.len());

        assert_eq!(
            downloaded, content_bytes,
            "Downloaded content doesn't match"
        );
        tlog!("Download content verified");

        // Delete
        tlog!("Deleting file...");
        storage.delete(handle).await.expect("Delete failed");
        tlog!("Delete complete");

        // Verify deleted
        tlog!("Verifying deletion...");
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
        tlog!("Deletion verified - object no longer exists");

        tlog!("Upload/download/delete test PASSED");
    }

    /// Test creating and deleting a folder.
    #[tokio::test]
    #[ignore] // Requires real MTP device - WRITES TO DEVICE
    #[serial]
    async fn test_create_delete_folder() {
        let _guard = TestGuard::new("test_create_delete_folder");

        tlog!("Opening device...");
        let device = MtpDevice::open_first().await.unwrap();
        tlog!("Device opened: {}", device.device_info().model);

        tlog!("Getting storages...");
        let storages = device.storages().await.unwrap();
        let storage = &storages[0];
        tlog!("Using storage: {}", storage.info().description);

        // Find Download folder (Android doesn't allow creating folders in root)
        tlog!("Listing root folder to find Download...");
        let root_objects = storage.list_objects(None).await.unwrap();
        let download_folder = root_objects
            .iter()
            .find(|o| o.filename == "Download")
            .expect("Download folder not found");
        tlog!(
            "Using Download folder (handle: {:?})",
            download_folder.handle
        );

        let folder_name = format!("mtp-rs-test-{}", std::process::id());
        tlog!("Creating folder: {}", folder_name);

        // Create folder inside Download
        let handle = storage
            .create_folder(Some(download_folder.handle), &folder_name)
            .await
            .expect("Create folder failed");

        tlog!("Folder created with handle: {:?}", handle);

        // Verify it exists
        tlog!("Verifying folder exists...");
        let info = storage.get_object_info(handle).await.unwrap();
        assert!(info.is_folder());
        assert_eq!(info.filename, folder_name);
        tlog!("Folder verified: {} (is_folder={})", info.filename, info.is_folder());

        // Delete it
        tlog!("Deleting folder...");
        storage.delete(handle).await.expect("Delete folder failed");
        tlog!("Folder deleted");

        tlog!("Folder create/delete test PASSED");
    }

    /// Test renaming a file.
    #[tokio::test]
    #[ignore] // Requires real MTP device - WRITES TO DEVICE
    #[serial]
    async fn test_rename_file() {
        let _guard = TestGuard::new("test_rename_file");

        tlog!("Opening device...");
        let device = MtpDevice::open_first().await.unwrap();
        tlog!("Device opened: {}", device.device_info().model);

        // Check if rename is supported
        tlog!("Checking if device supports rename...");
        if !device.supports_rename() {
            tlog!("Device does not support renaming (SetObjectPropValue not advertised)");
            tlog!("Skipping rename test");
            return;
        }
        tlog!("Device supports rename operation");

        tlog!("Getting storages...");
        let storages = device.storages().await.unwrap();
        let storage = &storages[0];
        tlog!("Using storage: {}", storage.info().description);

        // Find Download folder (Android doesn't allow creating files in root)
        tlog!("Listing root folder to find Download...");
        let root_objects = storage.list_objects(None).await.unwrap();
        let download_folder = root_objects
            .iter()
            .find(|o| o.filename == "Download")
            .expect("Download folder not found");
        tlog!(
            "Using Download folder (handle: {:?})",
            download_folder.handle
        );

        // Create a test file
        let original_name = format!("mtp-rs-rename-test-{}.txt", std::process::id());
        let renamed_name = format!("mtp-rs-renamed-{}.txt", std::process::id());
        let test_content = "Test file for rename operation";
        let content_bytes = test_content.as_bytes();

        tlog!(
            "Creating test file: {} ({} bytes)",
            original_name,
            content_bytes.len()
        );

        let info = NewObjectInfo::file(&original_name, content_bytes.len() as u64);
        let data_stream = futures::stream::iter(vec![Ok::<_, std::io::Error>(Bytes::from(
            content_bytes.to_vec(),
        ))]);

        let handle = storage
            .upload(Some(download_folder.handle), info, Box::pin(data_stream))
            .await
            .expect("Upload failed");

        tlog!("File created with handle: {:?}", handle);

        // Verify original name
        tlog!("Verifying original filename...");
        let info = storage.get_object_info(handle).await.unwrap();
        assert_eq!(info.filename, original_name);
        tlog!("Original filename verified: {}", info.filename);

        // Rename the file
        tlog!("Renaming {} -> {}", original_name, renamed_name);
        match storage.rename(handle, &renamed_name).await {
            Ok(()) => {
                tlog!("Rename operation completed");

                // Verify the new name
                tlog!("Verifying new filename...");
                let info = storage.get_object_info(handle).await.unwrap();
                assert_eq!(
                    info.filename, renamed_name,
                    "Filename should be updated after rename"
                );
                tlog!("New filename verified: {}", info.filename);
            }
            Err(Error::Protocol {
                code: mtp_rs::ptp::ResponseCode::OperationNotSupported,
                ..
            }) => {
                tlog!("Rename operation not supported by device (despite being advertised)");
                tlog!("This can happen with some Android devices");
            }
            Err(e) => {
                tlog!("Rename failed with error: {:?}", e);
                // Clean up before failing
                tlog!("Cleaning up: deleting test file...");
                let _ = storage.delete(handle).await;
                panic!("Rename failed: {:?}", e);
            }
        }

        // Clean up: delete the file
        tlog!("Cleaning up: deleting test file...");
        storage.delete(handle).await.expect("Delete failed");
        tlog!("Test file deleted");

        tlog!("Rename test PASSED");
    }
}
