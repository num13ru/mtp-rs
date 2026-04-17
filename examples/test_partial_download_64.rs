//! Test `GetPartialObject64` on a real device with a large (>4 GB) file.
//!
//! Usage:
//!   1. Connect your phone via USB, make sure it's in MTP mode
//!   2. Have a sample file at `/tmp/sample.mkv` larger than 4 GB
//!   3. Run: `cargo run --example test_partial_download_64`
//!
//! This:
//!   - Uploads the sample file to the phone's Download folder (or first writable folder)
//!   - Reads chunks at strategic offsets using `download_partial_64`
//!   - Compares each chunk byte-by-byte against the local source file
//!   - Demonstrates that `download_partial` (32-bit) produces wrong bytes beyond 4 GB
//!   - Deletes the uploaded file

use bytes::Bytes;
use futures::stream;
use mtp_rs::mtp::{MtpDevice, NewObjectInfo};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const SAMPLE_PATH: &str = "/tmp/sample.mkv";
const CHUNK_SIZE: u32 = 64 * 1024; // 64 KB — enough to catch any offset-related bug

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== GetPartialObject64 real-device test ===\n");

    // 1. Check the sample file.
    let sample = Path::new(SAMPLE_PATH);
    let file_meta = std::fs::metadata(sample)?;
    let file_size = file_meta.len();
    println!(
        "Local sample: {} ({} bytes = {:.2} GB)",
        SAMPLE_PATH,
        file_size,
        file_size as f64 / 1_073_741_824.0
    );
    if file_size <= u32::MAX as u64 {
        return Err(format!(
            "sample file is only {} bytes — need > 4 GB ({}) to test 64-bit offsets",
            file_size,
            u32::MAX
        )
        .into());
    }

    // 2. Connect.
    let device = MtpDevice::open_first().await?;
    println!(
        "Connected: {} {}\n",
        device.device_info().manufacturer,
        device.device_info().model
    );

    // Verify the device advertises GetPartialObject64.
    let supported = &device.device_info().operations_supported;
    let gpo64 = mtp_rs::ptp::OperationCode::GetPartialObject64;
    if supported.contains(&gpo64) {
        println!("✓ Device advertises GetPartialObject64");
    } else {
        println!(
            "⚠️  Warning: device does NOT advertise GetPartialObject64 (0x95C1) in its \
             supported ops list. The test will likely fail with OperationNotSupported."
        );
    }

    // 3. Upload the file.
    let storage = device
        .storages()
        .await?
        .into_iter()
        .next()
        .ok_or("no storage")?;
    println!(
        "Uploading to {} ({:.2} GB free)...",
        storage.info().description,
        storage.info().free_space_bytes as f64 / 1_073_741_824.0
    );

    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let remote_name = format!("mtp-rs-partial64-test-{}.mkv", ts);

    // Android devices typically reject uploads directly to the storage root.
    // Find an existing writable system folder (Download) to upload into.
    let root_objects = storage.list_objects(None).await?;
    let download_folder = root_objects
        .iter()
        .find(|o| o.is_folder() && o.filename == "Download")
        .ok_or("couldn't find Download folder on device")?;
    println!(
        "Using Download folder (handle {:?})",
        download_folder.handle
    );

    let info = NewObjectInfo::file(&remote_name, file_size);
    let upload_start = Instant::now();
    let upload_stream = file_stream(SAMPLE_PATH, 1024 * 1024)?;
    let handle = storage
        .upload(Some(download_folder.handle), info, upload_stream)
        .await?;
    let upload_secs = upload_start.elapsed().as_secs_f64();
    println!(
        "✓ Uploaded in {:.1}s ({:.1} MB/s) — handle {:?}\n",
        upload_secs,
        file_size as f64 / 1_048_576.0 / upload_secs,
        handle
    );

    // 4. Test partial reads at strategic offsets.
    let test_offsets: &[(u64, &str)] = &[
        (0, "start of file"),
        (2 * 1024 * 1024 * 1024, "2 GB (within 32-bit range)"),
        (
            4 * 1024 * 1024 * 1024 + 100 * 1024 * 1024,
            "4.1 GB (just past 32-bit boundary)",
        ),
        (file_size - CHUNK_SIZE as u64, "near EOF"),
    ];

    let mut all_passed = true;
    let mut local = std::fs::File::open(sample)?;
    for &(offset, label) in test_offsets {
        if offset + CHUNK_SIZE as u64 > file_size {
            println!(
                "Skipping offset {} ({}): would exceed file size",
                offset, label
            );
            continue;
        }
        println!("Offset {} bytes ({}):", offset, label);

        // Read expected bytes locally.
        local.seek(SeekFrom::Start(offset))?;
        let mut expected = vec![0u8; CHUNK_SIZE as usize];
        local.read_exact(&mut expected)?;

        // Download via GetPartialObject64.
        let t = Instant::now();
        let downloaded = storage
            .download_partial_64(handle, offset, CHUNK_SIZE)
            .await?;
        let ms = t.elapsed().as_millis();

        if downloaded == expected {
            println!("  ✓ {} bytes match ({} ms)", downloaded.len(), ms);
        } else {
            all_passed = false;
            println!(
                "  ✗ MISMATCH: got {} bytes, first diff at byte {}",
                downloaded.len(),
                first_diff(&downloaded, &expected)
            );
        }
    }

    // 5. Demonstrate that the 32-bit version is broken beyond 4 GB.
    let high_offset = 4u64 * 1024 * 1024 * 1024 + 100 * 1024 * 1024;
    if high_offset + CHUNK_SIZE as u64 <= file_size {
        println!("\nNow testing the 32-bit download_partial at offset {} (demonstrating the bug we're fixing):", high_offset);
        local.seek(SeekFrom::Start(high_offset))?;
        let mut expected = vec![0u8; CHUNK_SIZE as usize];
        local.read_exact(&mut expected)?;

        match storage
            .download_partial(handle, high_offset, CHUNK_SIZE)
            .await
        {
            Ok(data) if data == expected => {
                println!(
                    "  Unexpected: 32-bit op returned correct bytes (device may silently upgrade?)"
                );
            }
            Ok(data) => {
                println!(
                    "  As expected: 32-bit op returned {} bytes that do NOT match (first diff at {}). \
                     The 64-bit op is needed for offsets beyond 4 GB.",
                    data.len(),
                    first_diff(&data, &expected)
                );
            }
            Err(e) => {
                println!("  As expected: 32-bit op failed with: {}", e);
            }
        }
    }

    // 6. Cleanup.
    println!("\nCleaning up...");
    storage.delete(handle).await?;
    println!("✓ Deleted {}", remote_name);

    if all_passed {
        println!("\n🎉 All partial-download tests passed!");
    } else {
        println!("\n❌ Some tests failed.");
        std::process::exit(1);
    }
    Ok(())
}

/// True streaming file reader — reads the file one chunk at a time as the stream is consumed.
/// Avoids loading multi-GB files into RAM.
fn file_stream(
    path: &str,
    chunk_size: usize,
) -> std::io::Result<impl futures::Stream<Item = Result<Bytes, std::io::Error>> + Unpin> {
    let file = std::fs::File::open(path)?;
    let state = Mutex::new(file);
    Ok(Box::pin(stream::unfold(state, move |state| async move {
        let mut buf = vec![0u8; chunk_size];
        let result = {
            let mut file = state.lock().unwrap();
            file.read(&mut buf)
        };
        match result {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok(Bytes::from(buf)), state))
            }
            Err(e) => Some((Err(e), state)),
        }
    })))
}

fn first_diff(a: &[u8], b: &[u8]) -> usize {
    a.iter()
        .zip(b.iter())
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len()))
}
