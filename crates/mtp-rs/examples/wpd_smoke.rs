//! Manual smoke test for the Windows WPD backend against a **real device**.
//!
//! Drives the public `mtp::` API with `Backend::Wpd` forced, so it exercises the actual
//! `WpdBackend` read path (not the USB backend): open → device info → capabilities → storages →
//! list root → descend a folder → stream-download one file → verify its byte count against
//! `object_info`. The library-level analogue of the Phase 0 spike.
//!
//! Windows-only; needs a phone connected in MTP/File-transfer mode, screen unlocked.
//! Run: `cargo run -p mtp-rs --example wpd_smoke`

#[cfg(windows)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use mtp_rs::{Backend, ByteRange, MtpDevice, ObjectHandle, ObjectInfo, Storage};

    let device = MtpDevice::builder()
        .backend(Backend::Wpd)
        .open_first()
        .await?;

    let info = device.device_info();
    println!(
        "Device: {} {} (serial {:?}, version {:?})",
        info.manufacturer, info.model, info.serial_number, info.device_version
    );
    println!("Capabilities: {:?}\n", device.capabilities());

    let storages = device.storages().await?;
    println!("{} storage(s):", storages.len());

    // Recursively find the first downloadable file under a storage (bounded depth).
    async fn first_file(
        storage: &Storage,
        parent: Option<ObjectHandle>,
        depth: u32,
    ) -> Option<ObjectInfo> {
        let listing = storage.list_objects(parent).await.ok()?;
        if let Some(f) = listing.iter().find(|o| o.is_file() && o.size > 0) {
            return Some(f.clone());
        }
        if depth == 0 {
            return None;
        }
        for folder in listing.iter().filter(|o| o.is_folder()) {
            if let Some(f) = Box::pin(first_file(storage, Some(folder.handle), depth - 1)).await {
                return Some(f);
            }
        }
        None
    }

    for storage in &storages {
        let si = storage.info();
        println!(
            "\n== Storage: {} (cap={} free={} writable={}) ==",
            si.description, si.total_capacity, si.free_space, si.is_writable
        );

        let root = storage.list_objects(None).await?;
        println!("  root has {} object(s):", root.len());
        for o in root.iter().take(25) {
            let kind = if o.is_folder() { "[D]" } else { "[F]" };
            println!("    {kind} {:<32} {} bytes", o.filename, o.size);
        }

        // Download the first file we can find and verify its length.
        if let Some(file) = first_file(storage, None, 4).await {
            println!(
                "\n  downloading: {} (handle {:?}, size {})",
                file.filename, file.handle, file.size
            );
            let dl = storage.download(file.handle, ByteRange::Full).await?;
            let reported = dl.size();
            let bytes = dl.collect().await?;
            let verdict = if bytes.len() as u64 == reported && reported == file.size {
                "MATCH ✓"
            } else {
                "MISMATCH ✗"
            };
            println!(
                "  downloaded {} bytes (FileDownload::size()={}, object_info.size={}): {verdict}",
                bytes.len(),
                reported,
                file.size
            );

            // Spot-check the ranged/buffered primitive too.
            if file.size >= 16 {
                let mid = storage.read_range(file.handle, 8, 8).await?;
                println!(
                    "  read_range(8,8) returned {} bytes: {:02x?}",
                    mid.len(),
                    mid
                );
            }
        } else {
            println!("  (no downloadable file found under this storage)");
        }
    }

    device.close().await?;
    println!("\nDone.");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("wpd_smoke is Windows-only.");
}
