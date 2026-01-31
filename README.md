# mtp-rs

A pure-Rust library for talking to Android devices over MTP (Media Transfer Protocol).

No C dependencies. No libmtp. Just Rust and USB.

## What it does

- Connect to Android phones/tablets over USB
- List, download, upload, delete, move, and copy files
- Create and delete folders
- Stream large file downloads with progress
- Listen for device events (file added, storage removed, etc.)

## What it doesn't do

- MTPZ (the DRM extension some old devices used)
- Playlists and metadata syncing
- Vendor-specific extensions
- Legacy device quirks (we target modern Android only)

## Quick start

```rust
use mtp_rs::mtp::MtpDevice;

#[tokio::main]
async fn main() -> Result<(), mtp_rs::Error> {
    // Connect to the first MTP device
    let device = MtpDevice::open_first().await?;

    println!("Connected to {} {}",
        device.device_info().manufacturer,
        device.device_info().model);

    // List storages (internal storage, SD card, etc.)
    for storage in device.storages().await? {
        println!("{}: {:.2} GB free",
            storage.info().description,
            storage.info().free_space_bytes as f64 / 1e9);

        // List files in root
        for file in storage.list_objects(None).await? {
            let icon = if file.is_folder() { "📁" } else { "📄" };
            println!("  {} {}", icon, file.filename);
        }
    }

    Ok(())
}
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
mtp-rs = "0.1"
```

You'll also need an async runtime. The library is runtime-agnostic, but tokio is the most common choice:

```toml
[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

### Platform notes

**Linux**: You may need udev rules to access USB devices without root. Create `/etc/udev/rules.d/99-mtp.rules`:

```
SUBSYSTEM=="usb", ATTR{idVendor}=="*",  MODE="0666"
```

Then run `sudo udevadm control --reload-rules`.

**macOS**: Should work out of the box. If you have Android File Transfer installed, you may need to quit it first (it grabs the device exclusively).

**Windows**: Should work, but hasn't been extensively tested yet.

## Examples

### Download a file

```rust
let storage = &device.storages().await?[0];

// Find a file
let files = storage.list_objects(None).await?;
let photo = files.iter().find(|f| f.filename == "photo.jpg").unwrap();

// Download it
let data = storage.download(photo.handle).await?.collect().await?;
std::fs::write("photo.jpg", data)?;
```

### Upload a file

```rust
use mtp_rs::mtp::NewObjectInfo;
use bytes::Bytes;

let content = std::fs::read("document.pdf")?;
let info = NewObjectInfo::file("document.pdf", content.len() as u64);

let stream = futures::stream::iter(vec![Ok::<_, std::io::Error>(Bytes::from(content))]);
let handle = storage.upload(None, info, Box::pin(stream)).await?;

println!("Uploaded with handle {:?}", handle);
```

### Download with progress

```rust
use futures::StreamExt;

let mut stream = storage.download(file.handle).await?;

while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    if let Some(total) = chunk.total_bytes {
        let pct = chunk.bytes_so_far * 100 / total;
        println!("{}%", pct);
    }
    // Process chunk.data...
}
```

### Listen for events

```rust
loop {
    match device.next_event().await {
        Ok(event) => match event {
            DeviceEvent::ObjectAdded { handle } => {
                println!("New file: {:?}", handle);
            }
            DeviceEvent::StoreRemoved { storage_id } => {
                println!("Storage unplugged: {:?}", storage_id);
            }
            _ => {}
        },
        Err(Error::Timeout) => continue,
        Err(Error::Disconnected) => break,
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## API overview

The library has two layers:

### High-level API (`mtp::`)

This is what most people want. Friendly types, automatic session management, streaming.

- `MtpDevice` - Connect to devices, get info, list storages
- `Storage` - File operations (list, download, upload, delete, move, copy)
- `DownloadStream` - Streaming downloads with progress
- `DeviceEvent` - Events from the device

### Low-level API (`ptp::`)

For when you need raw protocol access (cameras, debugging, weird edge cases).

- `PtpDevice` - Raw device connection
- `PtpSession` - Manual session control, raw operations
- `OperationCode`, `ResponseCode` - Protocol constants
- Container types for building/parsing protocol messages

## Android MTP behaviors

Android's MTP implementation has some quirks that this library handles automatically:

| Behavior | What happens | How we handle it |
|----------|--------------|------------------|
| **Recursive listing broken** | `ObjectHandle::ALL` returns incomplete results (folders only, no files) | Auto-detected; uses manual folder traversal instead |
| **Can't create in root** | Creating files/folders in storage root fails with `InvalidObjectHandle` | Use a subfolder like `Download/` as the parent |
| **Large responses span transfers** | Data >64KB comes in multiple USB transfers | Automatically reassembled before parsing |
| **Composite USB devices** | Most phones report as USB class 0 (composite) | We inspect interfaces to find MTP |

The library detects Android devices via the `"android.com"` vendor extension and applies appropriate handling automatically. You generally don't need to worry about these details.

**Tip**: When uploading files, use a known folder like `Download/` rather than the storage root:

```rust
// Find the Download folder
let objects = storage.list_objects(None).await?;
let download = objects.iter().find(|o| o.filename == "Download").unwrap();

// Upload to Download folder (not root)
storage.upload(Some(download.handle), file_info, data).await?;
```

## Tested devices

"Full support" really means "Full support, except for general Android quirks listed above".

| Device                              | Android | Notes           |
|-------------------------------------|---------|-----------------|
| Google Pixel 9 Pro XL               | 15      | Full support    |
| Samsung Galaxy S23 Ultra (SM-S918B) | 14      | No root listing |

**Samsung quirk**: Samsung devices return `InvalidObjectHandle` when listing the root folder with handle 0.
The library automatically detects this and falls back to recursive listing with filtering. This is transparent to users.

We welcome reports of other tested devices! Please open an issue or PR with your device model, Android version,
and any issues encountered.

## Why not libmtp?

libmtp is battle-tested and comprehensive, but:

- It's a C library with all the FFI pain that entails
- It has a massive device quirks database for hardware from 2006
- The API is synchronous and callback-heavy
- It pulls in libusb, libudev, and other system dependencies

mtp-rs is pure Rust, async-native, and targets modern Android devices that all behave the same way. If you need to support a weird MP3 player from 2008, use libmtp. If you're building a modern Android sync tool, mtp-rs might be a better fit.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT OR Apache-2.0, at your option.
