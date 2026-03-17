// libmtp-rs backend for benchmarking MTP file transfers via the C libmtp library.
//
// System dependency: requires the `libmtp` C library to be installed on the system.
//   - macOS:  `brew install libmtp`
//   - Debian: `apt install libmtp-dev`
//   - Fedora: `dnf install libmtp-devel`

use libmtp_rs::device::raw::detect_raw_devices;
use libmtp_rs::device::{MtpDevice, StorageSort};
use libmtp_rs::object::filetypes::Filetype;
use libmtp_rs::object::Object;
use libmtp_rs::storage::files::FileMetadata;
use libmtp_rs::storage::Parent;
use libmtp_rs::util::HandlerReturn;
use std::cell::Cell;

/// Backend that wraps `libmtp-rs` (C libmtp) for MTP device operations.
pub struct LibmtpBackend {
    device: MtpDevice,
    /// ID of the temporary benchmark folder on the device.
    bench_folder_id: u32,
}

impl LibmtpBackend {
    /// Connect to the first MTP device and create a temporary benchmark folder
    /// ("mtp-bench-tmp") under the root of the default storage.
    pub fn connect() -> Result<Self, Box<dyn std::error::Error>> {
        let raw_devices =
            detect_raw_devices().map_err(|e| format!("Failed to detect raw devices: {:?}", e))?;

        let raw = raw_devices.first().ok_or("No MTP devices found")?;

        let mut device = raw
            .open_uncached()
            .ok_or("Failed to open MTP device (open_uncached returned None)")?;

        device
            .update_storage(StorageSort::NotSorted)
            .map_err(|e| format!("Failed to update storage: {:?}", e))?;

        let pool = device.storage_pool();
        let (folder_id, _name) = pool
            .create_folder("mtp-bench-tmp", Parent::Root)
            .map_err(|e| format!("Failed to create benchmark folder: {:?}", e))?;

        Ok(Self {
            device,
            bench_folder_id: folder_id,
        })
    }

    /// Upload `data` as `filename` into the benchmark folder. Returns a file ID (u32).
    pub fn upload(&self, filename: &str, data: &[u8]) -> Result<u32, Box<dyn std::error::Error>> {
        let metadata = FileMetadata {
            file_size: data.len() as u64,
            file_name: filename,
            file_type: Filetype::Unknown,
            modification_date: chrono::Utc::now(),
        };

        let offset = Cell::new(0usize);
        let pool = self.device.storage_pool();
        let file = pool
            .send_file_from_handler(
                |buf: &mut [u8]| {
                    let pos = offset.get();
                    if pos >= data.len() {
                        return HandlerReturn::Ok(0);
                    }
                    let remaining = &data[pos..];
                    let to_copy = remaining.len().min(buf.len());
                    buf[..to_copy].copy_from_slice(&remaining[..to_copy]);
                    offset.set(pos + to_copy);
                    HandlerReturn::Ok(to_copy as u32)
                },
                Parent::Folder(self.bench_folder_id),
                metadata,
            )
            .map_err(|e| format!("Failed to upload file '{}': {:?}", filename, e))?;

        Ok(file.id())
    }

    /// Download the file with the given ID, returning all bytes.
    pub fn download(&self, file_id: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut result = Vec::new();
        let pool = self.device.storage_pool();
        pool.get_file_to_handler(file_id, |chunk: &[u8]| {
            result.extend_from_slice(chunk);
            HandlerReturn::Ok(chunk.len() as u32)
        })
        .map_err(|e| format!("Failed to download file {}: {:?}", file_id, e))?;

        Ok(result)
    }

    /// List files in the root of the storage. Returns the count.
    pub fn list_objects(&self) -> Result<usize, Box<dyn std::error::Error>> {
        let pool = self.device.storage_pool();
        let files = pool.files_and_folders(Parent::Root);
        Ok(files.len())
    }

    /// Delete a single object by ID.
    pub fn delete(&self, object_id: u32) -> Result<(), Box<dyn std::error::Error>> {
        let dummy = self.device.dummy_object(object_id);
        dummy
            .delete()
            .map_err(|e| format!("Failed to delete object {}: {:?}", object_id, e))?;
        Ok(())
    }

    /// Clean up the benchmark folder (deletes children first, then the folder).
    pub fn cleanup(self) -> Result<(), Box<dyn std::error::Error>> {
        // Delete children first — some devices don't support recursive folder deletion.
        let pool = self.device.storage_pool();
        let children = pool.files_and_folders(Parent::Folder(self.bench_folder_id));
        for child in children {
            let dummy = self.device.dummy_object(child.id());
            let _ = dummy.delete(); // best-effort
        }

        let dummy = self.device.dummy_object(self.bench_folder_id);
        dummy
            .delete()
            .map_err(|e| format!("Failed to delete benchmark folder: {:?}", e))?;
        Ok(())
    }

    /// Human-readable device description.
    pub fn device_description(&self) -> String {
        let manufacturer = self
            .device
            .manufacturer_name()
            .unwrap_or_else(|_| "Unknown".to_string());
        let model = self
            .device
            .model_name()
            .unwrap_or_else(|_| "Unknown".to_string());
        format!("{} {} (libmtp)", manufacturer, model)
    }
}
