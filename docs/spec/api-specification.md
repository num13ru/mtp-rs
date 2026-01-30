# mtp-rs API Specification

**Version**: 0.1.0
**Rust Version**: 1.75+
**Runtime**: Agnostic (futures-based, no runtime dependency)

This document defines the public API for `mtp-rs`, a pure-Rust MTP (Media Transfer Protocol) library targeting modern
Android devices.

---

## Crate Structure

```
mtp_rs
├── ptp          # Low-level PTP protocol (camera-focused)
├── mtp          # High-level MTP API (media-focused)
├── transport    # USB transport abstraction
└── error        # Error types
```

---

## Core Types

### Newtypes for Type Safety

```rust
/// 32-bit object handle assigned by the device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub u32);

/// 32-bit storage identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StorageId(pub u32);

/// 32-bit session identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u32);

/// 32-bit transaction identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransactionId(pub u32);

impl ObjectHandle {
    /// Root folder (parent = root means object is in storage root)
    pub const ROOT: Self = ObjectHandle(0x00000000);
    /// All objects (used in GetObjectHandles to list recursively)
    pub const ALL: Self = ObjectHandle(0xFFFFFFFF);
}

impl StorageId {
    /// All storages (used in GetObjectHandles to search all)
    pub const ALL: Self = StorageId(0xFFFFFFFF);
}

/// Date and time (no timezone, naive)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DateTime {
    pub year: u16,   // e.g., 2024
    pub month: u8,   // 1-12
    pub day: u8,     // 1-31
    pub hour: u8,    // 0-23
    pub minute: u8,  // 0-59
    pub second: u8,  // 0-59
}

impl DateTime {
    /// Parse from MTP datetime string "YYYYMMDDThhmmss"
    pub fn parse(s: &str) -> Option<Self>;

    /// Format as MTP datetime string
    pub fn format(&self) -> String;
}
```

---

## mtp Module (High-Level API)

### MtpDevice

The primary entry point for interacting with MTP devices.

```rust
pub struct MtpDevice {
    // Internal: wraps PtpSession
}

impl MtpDevice {
    /// Create a builder for configuring device options
    pub fn builder() -> MtpDeviceBuilder;

    /// Open the first available MTP device
    pub async fn open_first() -> Result<Self, Error>;

    /// Open a specific device by USB bus/address
    pub async fn open(bus: u8, address: u8) -> Result<Self, Error>;

    /// List all available MTP devices without opening them
    pub async fn list_devices() -> Result<Vec<DeviceInfo>, Error>;

    /// Get device information
    pub fn device_info(&self) -> &DeviceInfo;

    /// Get all storages on the device
    pub async fn storages(&self) -> Result<Vec<Storage>, Error>;

    /// Get a specific storage by ID
    pub async fn storage(&self, id: StorageId) -> Result<Storage, Error>;

    /// Subscribe to device events.
    ///
    /// Returns a broadcast stream - multiple calls return independent streams
    /// that each receive all events. Events are buffered (up to 100); if buffer
    /// is full, oldest events are dropped. If no one is listening, events are
    /// discarded.
    pub fn events(&self) -> impl Stream<Item=DeviceEvent>;

    /// Close the connection (also happens on drop)
    pub async fn close(self) -> Result<(), Error>;
}
```

### MtpDeviceBuilder

```rust
pub struct MtpDeviceBuilder {
    // Internal configuration
}

impl MtpDeviceBuilder {
    pub fn new() -> Self;

    /// Set operation timeout (default: 30 seconds)
    pub fn timeout(self, timeout: Duration) -> Self;

    /// Open the first available device
    pub async fn open_first(self) -> Result<MtpDevice, Error>;

    /// Open a specific device
    pub async fn open(self, bus: u8, address: u8) -> Result<MtpDevice, Error>;
}
```

### DeviceInfo

```rust
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub device_version: String,
    pub serial_number: String,
    pub vendor_extension_id: u32,
    pub vendor_extension_version: u16,
    pub vendor_extension_desc: String,
    pub functional_mode: u16,
    pub operations_supported: Vec<OperationCode>,
    pub events_supported: Vec<EventCode>,
    pub capture_formats: Vec<ObjectFormatCode>,
    pub playback_formats: Vec<ObjectFormatCode>,
}
```

### Storage

Represents a storage location on the device (internal storage, SD card, etc.).

**Ownership**: `Storage` owns an `Arc<MtpDeviceInner>` internally, so it can outlive
the original `MtpDevice` reference and be used from multiple tasks.

```rust
pub struct Storage {
    // Internal: holds Arc<MtpDeviceInner>
}

impl Storage {
    /// Storage identifier
    pub fn id(&self) -> StorageId;

    /// Storage information (cached, call refresh() to update)
    pub fn info(&self) -> &StorageInfo;

    /// Refresh storage info from device (updates free space, etc.)
    pub async fn refresh(&self) -> Result<(), Error>;

    /// List objects in a folder (None = root)
    pub async fn list_objects(
        &self,
        parent: Option<ObjectHandle>,
    ) -> Result<Vec<ObjectInfo>, Error>;

    /// List objects recursively
    pub async fn list_objects_recursive(
        &self,
        parent: Option<ObjectHandle>,
    ) -> Result<Vec<ObjectInfo>, Error>;

    /// Get object metadata by handle
    pub async fn get_object_info(
        &self,
        handle: ObjectHandle,
    ) -> Result<ObjectInfo, Error>;

    /// Download a file as a stream with progress
    pub fn download<'a>(
        &'a self,
        handle: ObjectHandle,
    ) -> DownloadStream<'a>;

    /// Download a partial file (byte range)
    pub fn download_partial<'a>(
        &'a self,
        handle: ObjectHandle,
        offset: u64,
        size: u32,
    ) -> DownloadStream<'a>;

    /// Download thumbnail
    pub async fn download_thumbnail(
        &self,
        handle: ObjectHandle,
    ) -> Result<Vec<u8>, Error>;

    /// Upload a file from a stream
    pub async fn upload<S>(
        &self,
        parent: Option<ObjectHandle>,
        info: NewObjectInfo,
        data: S,
    ) -> Result<ObjectHandle, Error>
    where
        S: Stream<Item=Result<Bytes, std::io::Error>> + Unpin;

    /// Upload a file with progress callback
    pub async fn upload_with_progress<S, F>(
        &self,
        parent: Option<ObjectHandle>,
        info: NewObjectInfo,
        data: S,
        on_progress: F,
    ) -> Result<ObjectHandle, Error>
    where
        S: Stream<Item=Result<Bytes, std::io::Error>> + Unpin,
        F: FnMut(Progress) -> ControlFlow<()>;

    /// Create a folder
    pub async fn create_folder(
        &self,
        parent: Option<ObjectHandle>,
        name: &str,
    ) -> Result<ObjectHandle, Error>;

    /// Delete an object
    pub async fn delete(&self, handle: ObjectHandle) -> Result<(), Error>;

    /// Move an object to a different folder.
    /// The object handle remains valid after the move.
    pub async fn move_object(
        &self,
        handle: ObjectHandle,
        new_parent: ObjectHandle,
        new_storage: Option<StorageId>,
    ) -> Result<(), Error>;

    /// Copy an object
    pub async fn copy_object(
        &self,
        handle: ObjectHandle,
        new_parent: ObjectHandle,
        new_storage: Option<StorageId>,
    ) -> Result<ObjectHandle, Error>;
}
```

#### Important Behaviors and Limitations

**Upload cancellation**: If an upload is cancelled (via progress callback returning
`ControlFlow::Break` or by dropping the future), the partial file **will remain on
the device**. MTP has no "abort upload" operation. The caller should delete the
incomplete object if cleanup is needed.

**Upload size mismatch**: The `size` in `NewObjectInfo` must exactly match the bytes
sent. If the stream produces fewer or more bytes, device behavior is undefined
(typically an error).

**Delete non-empty folders**: Deleting a non-empty folder will fail with
`Access_Denied` or similar error. Use recursive listing + delete if needed.

**Files >4GB**: Not fully supported in v1 due to Android's broken property operations.
`ObjectInfo.size` will report 4GB (0xFFFFFFFF) for larger files. This is a known
limitation.

**Stream cancellation**: If a `DownloadStream` is dropped mid-transfer, remaining
data is drained in the background to maintain protocol consistency. This may take
time for large files.

### StorageInfo

```rust
#[derive(Debug, Clone)]
pub struct StorageInfo {
    pub id: StorageId,
    pub storage_type: StorageType,
    pub filesystem_type: FilesystemType,
    pub access_capability: AccessCapability,
    pub max_capacity: u64,
    pub free_space_bytes: u64,
    pub free_space_objects: u32,
    pub description: String,
    pub volume_identifier: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    Undefined,
    FixedRom,
    RemovableRom,
    FixedRam,
    RemovableRam,
    Unknown(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemType {
    Undefined,
    GenericFlat,
    GenericHierarchical,
    Dcf,
    Unknown(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessCapability {
    ReadWrite,
    ReadOnlyWithoutDeletion,
    ReadOnlyWithDeletion,
    Unknown(u16),
}
```

### ObjectInfo

```rust
#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub handle: ObjectHandle,
    pub storage_id: StorageId,
    pub format: ObjectFormat,
    pub protection_status: ProtectionStatus,
    pub size: u64,
    pub thumb_format: ObjectFormat,
    pub thumb_size: u32,
    pub thumb_width: u32,
    pub thumb_height: u32,
    pub image_width: u32,
    pub image_height: u32,
    pub image_bit_depth: u32,
    pub parent: ObjectHandle,
    pub association_type: AssociationType,
    pub association_desc: u32,
    pub filename: String,
    pub created: Option<DateTime>,
    pub modified: Option<DateTime>,
    pub keywords: String,
}

impl ObjectInfo {
    /// Check if this object is a folder
    pub fn is_folder(&self) -> bool;

    /// Check if this object is a file
    pub fn is_file(&self) -> bool;
}
```

### NewObjectInfo

Used when uploading new files.

**Note**: Filename must be ≤254 characters and cannot contain `/`, `\`, or null bytes.
The `size` field must exactly match the number of bytes that will be sent.

```rust
#[derive(Debug, Clone)]
pub struct NewObjectInfo {
    pub filename: String,
    pub size: u64,
    /// If None, auto-detected from filename extension (defaults to Undefined)
    pub format: Option<ObjectFormat>,
    pub modified: Option<DateTime>,
}

impl NewObjectInfo {
    /// Create info for a file. Format auto-detected from extension.
    pub fn file(filename: impl Into<String>, size: u64) -> Self;

    /// Create info for a folder.
    pub fn folder(name: impl Into<String>) -> Self;

    /// Create info with explicit format.
    pub fn with_format(filename: impl Into<String>, size: u64, format: ObjectFormat) -> Self;
}
```

### ObjectFormat

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectFormat {
    Undefined,
    Association,  // Folder
    Script,
    Executable,
    Text,
    Html,
    Dpof,
    Aiff,
    Wav,
    Mp3,
    Avi,
    Mpeg,
    Asf,
    Jpeg,
    Tiff,
    Bmp,
    Gif,
    Pict,
    Png,
    // ... MTP extensions
    WmaAudio,
    OggAudio,
    AacAudio,
    FlacAudio,
    Mp4Container,
    M4aAudio,
    WmvVideo,
    // Generic fallback
    Unknown(u16),
}

impl ObjectFormat {
    pub fn from_code(code: u16) -> Self;
    pub fn to_code(self) -> u16;
    pub fn from_extension(ext: &str) -> Self;
    pub fn is_audio(&self) -> bool;
    pub fn is_video(&self) -> bool;
    pub fn is_image(&self) -> bool;
}
```

### Streaming Downloads

```rust
/// A stream of file chunks during download
pub struct DownloadStream<'a> {
    // Internal
}

impl<'a> Stream for DownloadStream<'a> {
    type Item = Result<DownloadChunk, Error>;
}

impl<'a> DownloadStream<'a> {
    /// Total file size (if known)
    pub fn total_size(&self) -> Option<u64>;

    /// Collect all chunks into a Vec<u8>
    pub async fn collect(self) -> Result<Vec<u8>, Error>;

    /// Write to an AsyncWrite
    pub async fn write_to<W: AsyncWrite + Unpin>(
        self,
        writer: W,
    ) -> Result<u64, Error>;

    /// Write to a file path
    pub async fn write_to_file(
        self,
        path: impl AsRef<Path>,
    ) -> Result<u64, Error>;
}

#[derive(Debug)]
pub struct DownloadChunk {
    pub data: Bytes,
    pub bytes_so_far: u64,
    pub total_bytes: Option<u64>,
}
```

### Progress Tracking

```rust
#[derive(Debug, Clone)]
pub struct Progress {
    pub bytes_transferred: u64,
    pub total_bytes: Option<u64>,
}

impl Progress {
    /// Progress as a percentage (0.0 to 100.0), if total is known
    pub fn percent(&self) -> Option<f64>;

    /// Progress as a fraction (0.0 to 1.0), if total is known
    pub fn fraction(&self) -> Option<f64>;
}
```

### Device Events

```rust
/// Stream of events from the device
#[derive(Debug, Clone)]
pub enum DeviceEvent {
    /// A new object was added
    ObjectAdded { handle: ObjectHandle },

    /// An object was removed
    ObjectRemoved { handle: ObjectHandle },

    /// A storage was added (e.g., SD card inserted)
    StoreAdded { storage_id: StorageId },

    /// A storage was removed
    StoreRemoved { storage_id: StorageId },

    /// Storage info changed (e.g., free space)
    StorageInfoChanged { storage_id: StorageId },

    /// Object info changed
    ObjectInfoChanged { handle: ObjectHandle },

    /// Device info changed
    DeviceInfoChanged,

    /// Device is being reset
    DeviceReset,

    /// Unknown event
    Unknown { code: u16, params: [u32; 3] },
}
```

---

## ptp Module (Low-Level API)

For camera support and advanced use cases.

### PtpDevice

```rust
pub struct PtpDevice {
    // Internal: wraps transport
}

impl PtpDevice {
    /// Open a PTP device
    pub async fn open(bus: u8, address: u8) -> Result<Self, Error>;

    /// Open a PTP session
    pub async fn open_session(&self) -> Result<PtpSession, Error>;

    /// Get device info without opening a session
    pub async fn get_device_info(&self) -> Result<DeviceInfo, Error>;
}

pub struct PtpSession {
    // Internal
}

impl PtpSession {
    /// Session ID
    pub fn session_id(&self) -> SessionId;

    /// Execute a raw PTP operation
    pub async fn execute(
        &self,
        operation: OperationCode,
        params: &[u32],
    ) -> Result<Response, Error>;

    /// Execute operation with data phase (send)
    pub async fn execute_with_send(
        &self,
        operation: OperationCode,
        params: &[u32],
        data: &[u8],
    ) -> Result<Response, Error>;

    /// Execute operation with data phase (receive)
    pub async fn execute_with_receive(
        &self,
        operation: OperationCode,
        params: &[u32],
    ) -> Result<(Response, Vec<u8>), Error>;

    /// Stream data from device
    pub fn execute_with_receive_stream(
        &self,
        operation: OperationCode,
        params: &[u32],
    ) -> impl Stream<Item=Result<Bytes, Error>>;

    /// High-level operations
    pub async fn get_storage_ids(&self) -> Result<Vec<StorageId>, Error>;
    pub async fn get_storage_info(&self, id: StorageId) -> Result<StorageInfo, Error>;
    pub async fn get_object_handles(
        &self,
        storage: StorageId,
        format: Option<ObjectFormatCode>,
        parent: Option<ObjectHandle>,
    ) -> Result<Vec<ObjectHandle>, Error>;
    pub async fn get_object_info(&self, handle: ObjectHandle) -> Result<ObjectInfo, Error>;
    pub async fn get_object(&self, handle: ObjectHandle) -> Result<Vec<u8>, Error>;
    pub async fn get_partial_object(
        &self,
        handle: ObjectHandle,
        offset: u64,
        size: u32,
    ) -> Result<Vec<u8>, Error>;
    pub async fn get_thumb(&self, handle: ObjectHandle) -> Result<Vec<u8>, Error>;
    pub async fn send_object_info(
        &self,
        storage: StorageId,
        parent: ObjectHandle,
        info: &ObjectInfo,
    ) -> Result<(StorageId, ObjectHandle, ObjectHandle), Error>;
    pub async fn send_object(&self, data: &[u8]) -> Result<(), Error>;
    pub async fn delete_object(&self, handle: ObjectHandle) -> Result<(), Error>;
    pub async fn move_object(
        &self,
        handle: ObjectHandle,
        storage: StorageId,
        parent: ObjectHandle,
    ) -> Result<(), Error>;
    pub async fn copy_object(
        &self,
        handle: ObjectHandle,
        storage: StorageId,
        parent: ObjectHandle,
    ) -> Result<ObjectHandle, Error>;

    /// Listen for events
    pub fn events(&self) -> impl Stream<Item=DeviceEvent>;

    /// Close session
    pub async fn close(self) -> Result<(), Error>;
}
```

### Response

```rust
#[derive(Debug, Clone)]
pub struct Response {
    pub code: ResponseCode,
    pub params: Vec<u32>,
}

impl Response {
    pub fn is_ok(&self) -> bool;
}
```

### Operation and Response Codes

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum OperationCode {
    GetDeviceInfo = 0x1001,
    OpenSession = 0x1002,
    CloseSession = 0x1003,
    GetStorageIds = 0x1004,
    GetStorageInfo = 0x1005,
    GetNumObjects = 0x1006,
    GetObjectHandles = 0x1007,
    GetObjectInfo = 0x1008,
    GetObject = 0x1009,
    GetThumb = 0x100A,
    DeleteObject = 0x100B,
    SendObjectInfo = 0x100C,
    SendObject = 0x100D,
    MoveObject = 0x1019,
    CopyObject = 0x101A,
    GetPartialObject = 0x101B,
    // ... vendor-specific
    Unknown(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ResponseCode {
    Ok = 0x2001,
    GeneralError = 0x2002,
    SessionNotOpen = 0x2003,
    InvalidTransactionId = 0x2004,
    OperationNotSupported = 0x2005,
    ParameterNotSupported = 0x2006,
    IncompleteTransfer = 0x2007,
    InvalidStorageId = 0x2008,
    InvalidObjectHandle = 0x2009,
    StoreFull = 0x200C,
    ObjectWriteProtected = 0x200D,
    StoreReadOnly = 0x200E,
    AccessDenied = 0x200F,
    NoThumbnailPresent = 0x2010,
    DeviceBusy = 0x2019,
    InvalidParentObject = 0x201A,
    InvalidParameter = 0x201D,
    SessionAlreadyOpen = 0x201E,
    TransactionCancelled = 0x201F,
    ObjectTooLarge = 0xA809,
    Unknown(u16),
}
```

---

## Error Handling

```rust
#[derive(Debug)]
pub enum Error {
    /// USB communication error
    Usb(nusb::Error),

    /// Protocol-level error from device
    Protocol {
        code: ResponseCode,
        operation: OperationCode,
    },

    /// Invalid data received from device
    InvalidData {
        message: String,
    },

    /// I/O error
    Io(std::io::Error),

    /// Operation timed out
    Timeout,

    /// Device was disconnected
    Disconnected,

    /// Session not open
    SessionNotOpen,

    /// No device found
    NoDevice,

    /// Operation cancelled
    Cancelled,
}

impl std::error::Error for Error {}
impl std::fmt::Display for Error {}

impl Error {
    /// Check if this is a retryable error
    pub fn is_retryable(&self) -> bool;

    /// Get the response code if this is a protocol error
    pub fn response_code(&self) -> Option<ResponseCode>;
}
```

---

## Usage Examples

### List Files on Device

```rust
use mtp_rs::mtp::MtpDevice;

#[tokio::main]
async fn main() -> Result<(), mtp_rs::Error> {
    // Open the first MTP device
    let device = MtpDevice::open_first().await?;

    println!("Connected to: {} {}",
             device.device_info().manufacturer,
             device.device_info().model);

    // Get storages
    for storage in device.storages().await? {
        println!("Storage: {} ({} free)",
                 storage.info().description,
                 storage.info().free_space_bytes);

        // List root folder
        for obj in storage.list_objects(None).await? {
            let kind = if obj.is_folder() { "DIR " } else { "FILE" };
            println!("  {} {} ({} bytes)", kind, obj.filename, obj.size);
        }
    }

    Ok(())
}
```

### Download a File with Progress

```rust
use mtp_rs::mtp::MtpDevice;
use futures::StreamExt;

async fn download_file(
    storage: &Storage,
    handle: ObjectHandle,
    path: &str,
) -> Result<(), mtp_rs::Error> {
    let mut stream = storage.download(handle);
    let mut file = tokio::fs::File::create(path).await?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk.data).await?;

        if let Some(total) = chunk.total_bytes {
            println!("Progress: {:.1}%",
                     chunk.bytes_so_far as f64 / total as f64 * 100.0);
        }
    }

    Ok(())
}

// Or simply:
async fn download_simple(
    storage: &Storage,
    handle: ObjectHandle,
    path: &str,
) -> Result<(), mtp_rs::Error> {
    storage.download(handle).write_to_file(path).await?;
    Ok(())
}
```

### Upload a File

```rust
use mtp_rs::mtp::{MtpDevice, NewObjectInfo};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

async fn upload_file(
    storage: &Storage,
    local_path: &str,
    remote_folder: Option<ObjectHandle>,
) -> Result<ObjectHandle, mtp_rs::Error> {
    let file = File::open(local_path).await?;
    let metadata = file.metadata().await?;
    let filename = Path::new(local_path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let info = NewObjectInfo::file(filename, metadata.len());
    let stream = ReaderStream::new(file);

    storage.upload(remote_folder, info, stream).await
}
```

### Listen for Events

```rust
use mtp_rs::mtp::{MtpDevice, DeviceEvent};
use futures::StreamExt;

async fn watch_events(device: &MtpDevice) {
    let mut events = device.events();

    while let Some(event) = events.next().await {
        match event {
            DeviceEvent::ObjectAdded { handle } => {
                println!("New object: {:?}", handle);
            }
            DeviceEvent::ObjectRemoved { handle } => {
                println!("Removed object: {:?}", handle);
            }
            DeviceEvent::StoreAdded { storage_id } => {
                println!("New storage: {:?}", storage_id);
            }
            _ => {}
        }
    }
}
```

### Custom Timeout

```rust
use mtp_rs::mtp::MtpDevice;
use std::time::Duration;

let device = MtpDevice::builder()
.timeout(Duration::from_secs(60))
.open_first()
.await?;
```

---

## Thread Safety

- `MtpDevice` is `Send + Sync`
- `Storage` holds a reference to the device and is `Send + Sync`
- All async methods can be called from any task
- Internal synchronization uses async-aware primitives

---

## Cancellation

All async operations support cancellation via dropping:

```rust
// This will cancel the download if dropped
let download_future = storage.download(handle).collect();

// Cancel after timeout
tokio::select! {
    result = download_future => { /* completed */ }
    _ = tokio::time::sleep(Duration::from_secs(30)) => {
        // Future dropped, download cancelled
    }
}
```

---

## Known Limitations (v1)

| Limitation                  | Details                                                                                                                           |
|-----------------------------|-----------------------------------------------------------------------------------------------------------------------------------|
| **Files >4GB**              | Not fully supported. `ObjectInfo.size` will report 4GB (0xFFFFFFFF) for larger files due to Android's broken property operations. |
| **Filename length**         | Maximum 254 characters. Longer names are truncated.                                                                               |
| **Filename characters**     | Cannot contain `/`, `\`, or null bytes.                                                                                           |
| **Non-empty folder delete** | Deleting a non-empty folder fails. No automatic recursive delete.                                                                 |
| **Partial upload / resume** | Not supported in v1. Use chunked manual approach if needed.                                                                       |
| **Multiple sessions**       | Only one `MtpDevice` per physical device. Opening same device twice fails.                                                        |
| **MTP object properties**   | `GetObjectPropList`, `SetObjectPropList` not implemented due to Android bugs.                                                     |
| **Upload cancellation**     | Cancelled uploads may leave partial files on device.                                                                              |
| **Timezone handling**       | DateTime timezone suffixes are parsed but ignored; stored as naive time.                                                          |

---

## Error Recovery

**DeviceBusy**: Recommended retry pattern: 3 retries with 500ms delay. Not built into library.

**Disconnection**: After `Error::Disconnected`, the `MtpDevice` is no longer usable. All subsequent operations will
fail. Create a new connection.

**Retryable errors**:

```rust
if error.is_retryable() {
// Safe to retry: DeviceBusy, Timeout
}
```
