//! Diagnostic script to investigate MTP issues.
//!
//! Run with: cargo run --example diagnose

use futures::StreamExt;
use mtp_rs::mtp::MtpDevice;
use mtp_rs::ptp::ObjectHandle;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MTP Diagnostic Tool ===\n");

    // Connect to device
    let device = MtpDevice::open_first().await?;
    println!("Connected to: {} {}", device.device_info().manufacturer, device.device_info().model);

    let storages = device.storages().await?;
    let storage = &storages[0];
    println!("Storage: {}\n", storage.info().description);

    // Test 1: List root objects (non-recursive)
    println!("=== Test 1: Root folder listing (non-recursive) ===");
    let root_objects = storage.list_objects(None).await?;
    let root_folders = root_objects.iter().filter(|o| o.is_folder()).count();
    let root_files = root_objects.iter().filter(|o| o.is_file()).count();
    println!("Root contains: {} folders, {} files, {} total\n", root_folders, root_files, root_objects.len());

    // Test 2: List recursive
    println!("=== Test 2: Recursive listing (ObjectHandle::ALL) ===");
    let recursive_objects = storage.list_objects_recursive(None).await?;
    let rec_folders = recursive_objects.iter().filter(|o| o.is_folder()).count();
    let rec_files = recursive_objects.iter().filter(|o| o.is_file()).count();
    println!("Recursive contains: {} folders, {} files, {} total\n", rec_folders, rec_files, recursive_objects.len());

    // Test 3: Manual recursive listing of first folder
    if let Some(first_folder) = root_objects.iter().find(|o| o.is_folder()) {
        println!("=== Test 3: Listing contents of '{}' folder ===", first_folder.filename);
        let folder_contents = storage.list_objects(Some(first_folder.handle)).await?;
        let sub_folders = folder_contents.iter().filter(|o| o.is_folder()).count();
        let sub_files = folder_contents.iter().filter(|o| o.is_file()).count();
        println!("'{}' contains: {} folders, {} files, {} total\n",
            first_folder.filename, sub_folders, sub_files, folder_contents.len());

        // Show first few items
        for (i, obj) in folder_contents.iter().take(5).enumerate() {
            let kind = if obj.is_folder() { "DIR" } else { "FILE" };
            println!("  {}. {} {} ({} bytes)", i+1, kind, obj.filename, obj.size);
        }
        if folder_contents.len() > 5 {
            println!("  ... and {} more", folder_contents.len() - 5);
        }
        println!();
    }

    // Test 4: Find and download a small file
    println!("=== Test 4: Download test ===");
    let small_file = root_objects.iter()
        .filter(|o| o.is_file() && o.size > 1000 && o.size < 100_000)
        .next();

    match small_file {
        Some(file) => {
            println!("Downloading: {} ({} bytes)", file.filename, file.size);
            let stream = storage.download(file.handle).await?;
            let data: Vec<u8> = stream.collect().await?;
            println!("Downloaded {} bytes successfully!", data.len());

            // Verify size matches
            if data.len() as u64 == file.size {
                println!("✓ Size matches expected");
            } else {
                println!("✗ Size mismatch: expected {}, got {}", file.size, data.len());
            }
        }
        None => {
            println!("No suitable small file found in root, checking subfolders...");

            // Try to find a file in a subfolder
            for folder in root_objects.iter().filter(|o| o.is_folder()).take(5) {
                let contents = storage.list_objects(Some(folder.handle)).await?;
                if let Some(file) = contents.iter()
                    .filter(|o| o.is_file() && o.size > 1000 && o.size < 100_000)
                    .next()
                {
                    println!("Found file in '{}': {} ({} bytes)", folder.filename, file.filename, file.size);
                    let stream = storage.download(file.handle).await?;
                    let data: Vec<u8> = stream.collect().await?;
                    println!("Downloaded {} bytes successfully!", data.len());

                    if data.len() as u64 == file.size {
                        println!("✓ Size matches expected");
                    } else {
                        println!("✗ Size mismatch: expected {}, got {}", file.size, data.len());
                    }
                    break;
                }
            }
        }
    }

    println!("\n=== Diagnostics complete ===");
    Ok(())
}
