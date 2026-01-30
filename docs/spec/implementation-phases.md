# Implementation Phases

This document breaks down the `mtp-rs` implementation into phases. Each phase is independently testable and builds on
the previous one.

---

## Overview

```
Phase 1: Types & Serialization    ─┐
                                   ├── No USB, pure Rust
Phase 2: Protocol Layer           ─┘

Phase 3: USB Transport            ─── Requires nusb

Phase 4: MTP High-Level API       ─┐
                                   ├── Integration
Phase 5: PTP API & Events         ─┘

Phase 6: Integration Testing      ─── Requires real device
```

---

## Phase 1: Types & Serialization

**Goal**: Implement binary serialization/deserialization for all MTP/PTP data types.

**No dependencies on USB. Fully testable with unit tests.**

### Deliverables

```
src/
├── lib.rs                 # Crate skeleton
├── error.rs               # Error types (partial)
└── ptp/
    ├── mod.rs
    ├── codes.rs           # Operation, response, event code enums
    └── pack.rs            # Serialization module
```

### What to Implement

#### `ptp/codes.rs`

- `OperationCode` enum (all codes from protocol-reference.md)
- `ResponseCode` enum
- `EventCode` enum
- `ObjectFormatCode` enum
- Each with `from_code(u16)` and `to_code() -> u16` methods

#### `ptp/pack.rs`

Serialization functions:

```rust
// Primitives
pub fn pack_u8(val: u8) -> [u8; 1];
pub fn pack_u16(val: u16) -> [u8; 2];
pub fn pack_u32(val: u32) -> [u8; 4];
pub fn pack_u64(val: u64) -> [u8; 8];

pub fn unpack_u8(buf: &[u8]) -> Result<u8, Error>;
pub fn unpack_u16(buf: &[u8]) -> Result<u16, Error>;
pub fn unpack_u32(buf: &[u8]) -> Result<u32, Error>;
pub fn unpack_u64(buf: &[u8]) -> Result<u64, Error>;

// Strings (UTF-16LE with length prefix)
pub fn pack_string(s: &str) -> Vec<u8>;
pub fn unpack_string(buf: &[u8]) -> Result<(String, usize), Error>;

// Arrays
pub fn pack_u16_array(arr: &[u16]) -> Vec<u8>;
pub fn pack_u32_array(arr: &[u32]) -> Vec<u8>;
pub fn unpack_u16_array(buf: &[u8]) -> Result<(Vec<u16>, usize), Error>;
pub fn unpack_u32_array(buf: &[u8]) -> Result<(Vec<u32>, usize), Error>;

// DateTime (format: "YYYYMMDDThhmmss", timezone ignored)
pub fn pack_datetime(dt: &DateTime) -> Vec<u8>;
pub fn unpack_datetime(buf: &[u8]) -> Result<(Option<DateTime>, usize), Error>;

/// Naive date/time (no timezone)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}
```

### Success Criteria

- All serialization functions have unit tests
- Test with known byte sequences from MTP spec examples
- Round-trip tests: `unpack(pack(x)) == x`
- Fuzzing with `proptest` for edge cases

### Test Examples

```rust
#[test]
fn test_pack_u32() {
    assert_eq!(pack_u32(0x12345678), [0x78, 0x56, 0x34, 0x12]);
}

#[test]
fn test_pack_string() {
    // "Hi" -> length=3, 'H', 'i', null
    let packed = pack_string("Hi");
    assert_eq!(packed, vec![
        0x03,             // length (including null)
        0x48, 0x00,       // 'H'
        0x69, 0x00,       // 'i'
        0x00, 0x00,       // null terminator
    ]);
}

#[test]
fn test_pack_empty_string() {
    assert_eq!(pack_string(""), vec![0x00]);
}

#[test]
fn test_pack_u32_array() {
    let packed = pack_u32_array(&[1, 2, 3]);
    assert_eq!(packed, vec![
        0x03, 0x00, 0x00, 0x00,  // count = 3
        0x01, 0x00, 0x00, 0x00,  // 1
        0x02, 0x00, 0x00, 0x00,  // 2
        0x03, 0x00, 0x00, 0x00,  // 3
    ]);
}
```

---

## Phase 2: Protocol Layer

**Goal**: Implement container format and data structure serialization.

**Still no USB. Uses mock transport for testing.**

### Deliverables

```
src/ptp/
├── container.rs           # USB container format
├── types.rs               # DeviceInfo, StorageInfo, ObjectInfo
└── session.rs             # Session logic (partial, no transport)

src/transport/
├── mod.rs                 # Transport trait
└── mock.rs                # Mock transport for testing
```

### What to Implement

#### `ptp/container.rs`

```rust
pub struct CommandContainer {
    pub code: OperationCode,
    pub transaction_id: u32,
    pub params: Vec<u32>,  // 0-5 params
}

pub struct DataContainer {
    pub code: OperationCode,
    pub transaction_id: u32,
    pub payload: Vec<u8>,
}

pub struct ResponseContainer {
    pub code: ResponseCode,
    pub transaction_id: u32,
    pub params: Vec<u32>,  // 0-5 params
}

pub struct EventContainer {
    pub code: EventCode,
    pub transaction_id: u32,
    pub params: [u32; 3],
}

impl CommandContainer {
    pub fn to_bytes(&self) -> Vec<u8>;
}

impl ResponseContainer {
    pub fn from_bytes(buf: &[u8]) -> Result<Self, Error>;
}

impl DataContainer {
    pub fn to_bytes(&self) -> Vec<u8>;
    pub fn from_bytes(buf: &[u8]) -> Result<Self, Error>;
}

impl EventContainer {
    pub fn from_bytes(buf: &[u8]) -> Result<Self, Error>;
}
```

#### `ptp/types.rs`

```rust
pub struct DeviceInfo {
    /* fields */
}
pub struct StorageInfo {
    /* fields */
}
pub struct ObjectInfo {
    /* fields */
}

impl DeviceInfo {
    pub fn from_bytes(buf: &[u8]) -> Result<Self, Error>;
}

impl StorageInfo {
    pub fn from_bytes(buf: &[u8]) -> Result<Self, Error>;
}

impl ObjectInfo {
    pub fn from_bytes(buf: &[u8]) -> Result<Self, Error>;
    pub fn to_bytes(&self) -> Vec<u8>;  // For SendObjectInfo
}
```

#### `transport/mod.rs`

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn send_bulk(&self, data: &[u8]) -> Result<(), Error>;
    async fn receive_bulk(&self, max_size: usize) -> Result<Vec<u8>, Error>;
    async fn receive_interrupt(&self) -> Result<Vec<u8>, Error>;
}
```

#### `transport/mock.rs`

```rust
pub struct MockTransport {
    expected_sends: VecDeque<Vec<u8>>,
    queued_responses: VecDeque<Vec<u8>>,
    sent: Vec<Vec<u8>>,
}

impl MockTransport {
    pub fn new() -> Self;
    pub fn expect_send(&mut self, data: Vec<u8>);
    pub fn queue_response(&mut self, data: Vec<u8>);
    /// Verify all expected sends occurred AND all queued responses consumed
    pub fn verify(&self) -> Result<(), String>;
    /// Get all data that was sent (for inspection in tests)
    pub fn get_sends(&self) -> &[Vec<u8>];
}
```

### Success Criteria

- Container serialization matches spec byte-for-byte
- DeviceInfo, StorageInfo, ObjectInfo parse correctly
- Mock transport enables protocol testing without USB
- Property-based tests for all structures

### Test Examples

```rust
#[test]
fn test_command_container() {
    let cmd = CommandContainer {
        code: OperationCode::OpenSession,
        transaction_id: 1,
        params: vec![1],  // session ID
    };

    assert_eq!(cmd.to_bytes(), vec![
        0x10, 0x00, 0x00, 0x00,  // length = 16
        0x01, 0x00,              // type = Command
        0x02, 0x10,              // code = OpenSession
        0x01, 0x00, 0x00, 0x00,  // transaction_id = 1
        0x01, 0x00, 0x00, 0x00,  // param1 = 1
    ]);
}

#[test]
fn test_response_container_parse() {
    let bytes = vec![
        0x0c, 0x00, 0x00, 0x00,  // length = 12
        0x03, 0x00,              // type = Response
        0x01, 0x20,              // code = OK
        0x01, 0x00, 0x00, 0x00,  // transaction_id = 1
    ];

    let resp = ResponseContainer::from_bytes(&bytes).unwrap();
    assert_eq!(resp.code, ResponseCode::Ok);
    assert_eq!(resp.transaction_id, 1);
}
```

---

## Phase 3: USB Transport

**Goal**: Implement actual USB communication using nusb.

**Requires nusb crate. Testable with real devices.**

### Deliverables

```
src/transport/
└── nusb.rs                # Real USB transport

Cargo.toml                 # Add nusb dependency
```

### What to Implement

#### `transport/nusb.rs`

```rust
pub struct NusbTransport {
    interface: nusb::Interface,
    bulk_in: u8,
    bulk_out: u8,
    interrupt_in: u8,
    timeout: Duration,
}

impl NusbTransport {
    pub async fn open(device: nusb::Device) -> Result<Self, Error>;
    pub async fn list_mtp_devices() -> Result<Vec<nusb::DeviceInfo>, Error>;
}

impl Transport for NusbTransport {
    async fn send_bulk(&self, data: &[u8]) -> Result<(), Error>;
    async fn receive_bulk(&self, max_size: usize) -> Result<Vec<u8>, Error>;
    async fn receive_interrupt(&self) -> Result<Vec<u8>, Error>;
}
```

### USB Device Discovery

```rust
pub async fn list_mtp_devices() -> Result<Vec<nusb::DeviceInfo>, Error> {
    // MTP devices have:
    // - Class: 0x06 (Still Image) or 0xFF (Vendor-specific)
    // - Subclass: 0x01 (MTP)
    // - Protocol: 0x01 (PTP)
}
```

### Success Criteria

- Can enumerate MTP devices
- Can claim interface and identify endpoints
- Basic send/receive works
- Proper error handling for USB errors

### Test (Integration)

```rust
#[tokio::test]
#[ignore]  // Requires real device
async fn test_usb_transport() {
    let devices = NusbTransport::list_mtp_devices().await.unwrap();
    assert!(!devices.is_empty(), "No MTP devices found");

    let transport = NusbTransport::open(devices[0].open().unwrap()).await.unwrap();
    // Send GetDeviceInfo (no session required)
    // Verify response
}
```

---

## Phase 4: MTP High-Level API

**Goal**: Implement the user-facing MTP API.

### Deliverables

```
src/
├── ptp/
│   ├── session.rs         # Complete session implementation
│   └── device.rs          # PtpDevice (low-level API)
└── mtp/
    ├── mod.rs
    ├── device.rs          # MtpDevice, MtpDeviceBuilder
    ├── storage.rs         # Storage
    ├── object.rs          # ObjectInfo, NewObjectInfo, ObjectFormat
    └── stream.rs          # DownloadStream
```

### What to Implement

#### Complete `ptp/session.rs`

```rust
pub struct PtpSession {
    transport: Arc<dyn Transport>,
    session_id: SessionId,
    transaction_id: AtomicU32,
    lock: AsyncMutex<()>,
}

impl PtpSession {
    pub async fn open(transport: Arc<dyn Transport>, session_id: u32) -> Result<Self, Error>;
    pub async fn close(self) -> Result<(), Error>;

    // All operations from protocol-reference.md
    pub async fn get_device_info(&self) -> Result<DeviceInfo, Error>;
    pub async fn get_storage_ids(&self) -> Result<Vec<StorageId>, Error>;
    pub async fn get_storage_info(&self, id: StorageId) -> Result<StorageInfo, Error>;
    pub async fn get_object_handles(...) -> Result<Vec<ObjectHandle>, Error>;
    pub async fn get_object_info(&self, handle: ObjectHandle) -> Result<ObjectInfo, Error>;
    pub async fn get_object(&self, handle: ObjectHandle) -> Result<Vec<u8>, Error>;
    pub async fn get_partial_object(...) -> Result<Vec<u8>, Error>;
    pub async fn get_thumb(&self, handle: ObjectHandle) -> Result<Vec<u8>, Error>;
    pub async fn send_object_info(...) -> Result<(StorageId, ObjectHandle, ObjectHandle), Error>;
    pub async fn send_object(&self, data: &[u8]) -> Result<(), Error>;
    pub async fn delete_object(&self, handle: ObjectHandle) -> Result<(), Error>;
    pub async fn move_object(...) -> Result<(), Error>;
    pub async fn copy_object(...) -> Result<ObjectHandle, Error>;
}
```

#### `mtp/device.rs`, `mtp/storage.rs`

Implement the high-level API as specified in api-specification.md.

#### `mtp/stream.rs`

```rust
pub struct DownloadStream<'a> {
    session: &'a PtpSession,
    handle: ObjectHandle,
    offset: u64,
    total_size: Option<u64>,
    buffer: VecDeque<u8>,
}

impl Stream for DownloadStream<'_> {
    type Item = Result<DownloadChunk, Error>;
}
```

### Success Criteria

- All operations work with mock transport
- Streaming downloads yield chunks correctly
- Progress tracking is accurate
- Error handling is comprehensive

---

## Phase 5: PTP API & Events

**Goal**: Expose low-level PTP API and implement event handling.

### Deliverables

```
src/
├── ptp/
│   └── device.rs          # PtpDevice public API
└── mtp/
    └── event.rs           # DeviceEvent, event stream
```

### What to Implement

#### `ptp/device.rs`

```rust
pub struct PtpDevice {
    transport: Arc<NusbTransport>,
}

impl PtpDevice {
    pub async fn open(bus: u8, address: u8) -> Result<Self, Error>;
    pub async fn get_device_info(&self) -> Result<DeviceInfo, Error>;
    pub async fn open_session(&self) -> Result<PtpSession, Error>;
}
```

#### `mtp/event.rs`

```rust
pub fn event_stream(transport: Arc<dyn Transport>) -> impl Stream<Item=DeviceEvent> {
    // Spawn task to poll interrupt endpoint
    // Parse EventContainer
    // Yield DeviceEvent
}
```

### Success Criteria

- PtpDevice API matches specification
- Events are received and parsed correctly
- Event stream is cancellation-safe

---

## Phase 6: Integration Testing

**Goal**: Verify everything works with real devices.

### Test Scenarios

```rust
#[tokio::test]
#[ignore]
async fn test_list_storages() {
    let device = MtpDevice::open_first().await.unwrap();
    let storages = device.storages().await.unwrap();
    assert!(!storages.is_empty());

    for storage in &storages {
        println!("{}: {} bytes free",
                 storage.info().description,
                 storage.info().free_space_bytes);
    }
}

#[tokio::test]
#[ignore]
async fn test_list_files() {
    let device = MtpDevice::open_first().await.unwrap();
    let storage = &device.storages().await.unwrap()[0];
    let files = storage.list_objects(None).await.unwrap();

    // Should have at least some system folders
    assert!(files.iter().any(|f| f.is_folder()));
}

#[tokio::test]
#[ignore]
async fn test_upload_download_delete() {
    let device = MtpDevice::open_first().await.unwrap();
    let storage = &device.storages().await.unwrap()[0];

    // Upload a test file
    let content = b"Hello from mtp-rs!";
    let info = NewObjectInfo::file("mtp-rs-test.txt", content.len() as u64);
    let handle = storage.upload(None, info, futures::stream::once(async {
        Ok(Bytes::from_static(content))
    })).await.unwrap();

    // Download it back
    let downloaded = storage.download(handle).collect().await.unwrap();
    assert_eq!(downloaded, content);

    // Delete it
    storage.delete(handle).await.unwrap();
}
```

### Success Criteria

- All tests pass with real Android device
- Large file transfers work correctly
- Events are received
- No resource leaks

---

## Dependency Summary

| Phase | Dependencies  | Testable Without Device    |
|-------|---------------|----------------------------|
| 1     | None          | Yes                        |
| 2     | None          | Yes (mock transport)       |
| 3     | nusb          | No                         |
| 4     | nusb          | Partially (mock for logic) |
| 5     | nusb          | Partially                  |
| 6     | nusb + device | No                         |

---

## Estimated Complexity

| Phase     | Files  | Estimated Lines | Difficulty  |
|-----------|--------|-----------------|-------------|
| 1         | 3      | 500-700         | Low         |
| 2         | 4      | 800-1000        | Medium      |
| 3         | 1      | 300-400         | Medium      |
| 4         | 5      | 1000-1500       | Medium-High |
| 5         | 2      | 300-500         | Medium      |
| 6         | 1      | 200-300         | Low         |
| **Total** | **16** | **3100-4400**   |             |
