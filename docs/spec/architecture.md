# mtp-rs Architecture

This document describes the internal architecture, module organization, and design decisions for `mtp-rs`.

---

## Layer Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                      User Application                        │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                     Public API Layer                         │
│  ┌─────────────────────┐    ┌─────────────────────────┐     │
│  │   mtp::MtpDevice    │    │    ptp::PtpDevice       │     │
│  │   mtp::Storage      │    │    ptp::PtpSession      │     │
│  │   mtp::ObjectInfo   │    │                         │     │
│  │   (media-focused)   │    │    (camera-focused)     │     │
│  └──────────┬──────────┘    └────────────┬────────────┘     │
│             │                            │                   │
│             └────────────┬───────────────┘                   │
└──────────────────────────┼──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                    Protocol Layer                            │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                   ptp::Session                        │   │
│  │  - Transaction management                             │   │
│  │  - Operation execution                                │   │
│  │  - Response handling                                  │   │
│  │  - Event listening                                    │   │
│  └──────────────────────────┬───────────────────────────┘   │
│                             │                                │
│  ┌──────────────────────────▼───────────────────────────┐   │
│  │                   ptp::Container                      │   │
│  │  - Command/Data/Response/Event containers             │   │
│  │  - Serialization/deserialization                      │   │
│  └──────────────────────────┬───────────────────────────┘   │
│                             │                                │
│  ┌──────────────────────────▼───────────────────────────┐   │
│  │                   ptp::pack                           │   │
│  │  - Primitive type encoding (u16, u32, u64)            │   │
│  │  - String encoding (UTF-16LE)                         │   │
│  │  - Array encoding                                     │   │
│  │  - Dataset structures (DeviceInfo, ObjectInfo, etc.)  │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                    Transport Layer                           │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                 transport::Transport                  │   │
│  │  (trait)                                              │   │
│  │  - send_bulk(&[u8])                                   │   │
│  │  - receive_bulk() -> Vec<u8>                          │   │
│  │  - receive_interrupt() -> Event                       │   │
│  └──────────────────────────┬───────────────────────────┘   │
│                             │                                │
│  ┌──────────────────────────▼───────────────────────────┐   │
│  │              transport::NusbTransport                 │   │
│  │  - nusb device wrapper                                │   │
│  │  - Endpoint management                                │   │
│  │  - Async USB operations                               │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                       nusb crate                             │
│                   (USB device access)                        │
└─────────────────────────────────────────────────────────────┘
```

---

## Module Organization

```
src/
├── lib.rs                 # Crate root, re-exports
│
├── error.rs               # Error types
│
├── ptp/                   # PTP protocol implementation
│   ├── mod.rs             # Module exports
│   ├── codes.rs           # Operation, response, event codes
│   ├── container.rs       # USB container format
│   ├── pack.rs            # Binary serialization
│   ├── types.rs           # DeviceInfo, ObjectInfo, StorageInfo
│   ├── session.rs         # PTP session management
│   └── device.rs          # PtpDevice (public low-level API)
│
├── mtp/                   # MTP high-level API
│   ├── mod.rs             # Module exports
│   ├── device.rs          # MtpDevice, MtpDeviceBuilder
│   ├── storage.rs         # Storage
│   ├── object.rs          # ObjectInfo, NewObjectInfo, ObjectFormat
│   ├── event.rs           # DeviceEvent, event stream
│   └── stream.rs          # DownloadStream, upload helpers
│
└── transport/             # USB transport abstraction
    ├── mod.rs             # Transport trait, exports
    └── nusb.rs            # nusb implementation
```

---

## Dependency Rules

The layers have strict dependency rules to maintain separation:

```
┌─────────────┐
│    mtp      │ ──────────┐
└─────────────┘           │
       │                  │
       ▼                  ▼
┌─────────────┐    ┌─────────────┐
│    ptp      │◄───│   error     │
└─────────────┘    └─────────────┘
       │                  ▲
       ▼                  │
┌─────────────┐           │
│  transport  │───────────┘
└─────────────┘
```

- **mtp** depends on: `ptp`, `error`
- **ptp** depends on: `transport`, `error`
- **transport** depends on: `error`, external `nusb`
- **error** depends on: nothing internal

**Never**:

- `ptp` imports from `mtp`
- `transport` imports from `ptp` or `mtp`
- Circular dependencies

---

## Key Design Decisions

### 1. Two-Level API

**Decision**: Provide both high-level `mtp::` and low-level `ptp::` APIs.

**Rationale**:

- `mtp::` provides a clean, safe API for media device operations
- `ptp::` allows camera support and advanced use cases
- Users can drop down to `ptp::` when needed without reimplementing everything

### 2. Transport Trait Abstraction

**Decision**: Abstract USB communication behind a `Transport` trait.

```rust
pub trait Transport: Send + Sync {
    async fn send_bulk(&self, data: &[u8]) -> Result<(), Error>;
    async fn receive_bulk(&self, max_size: usize) -> Result<Vec<u8>, Error>;
    async fn receive_interrupt(&self) -> Result<Vec<u8>, Error>;
}
```

**Rationale**:

- Enables unit testing with mock transport
- Future-proofs for alternative backends if needed
- Clean separation of concerns

### 3. Storage as First-Class Object

**Decision**: Operations are methods on `Storage`, not `Device`.

```rust
// Yes
let files = storage.list_objects(None).await?;

// No
let files = device.list_objects(storage_id, None).await?;
```

**Rationale**:

- Prevents accidentally mixing up storage IDs
- More intuitive API
- Storage holds reference to device, enforces lifetime

### 4. Stream-Based Downloads

**Decision**: Downloads return `Stream<Item = Result<Chunk, Error>>`.

**Rationale**:

- Memory efficient for large files
- Natural progress tracking
- User controls buffering strategy
- Composable with async ecosystem

### 5. No Runtime Dependency

**Decision**: Library uses only `futures` traits, no tokio/async-std dependency.

**Rationale**:

- Works with any async runtime
- `nusb` is already runtime-agnostic
- Smaller dependency footprint

### 6. Newtype Wrappers for IDs

**Decision**: `ObjectHandle(u32)`, `StorageId(u32)` instead of raw `u32`.

**Rationale**:

- Prevents mixing up object handles and storage IDs
- Self-documenting code
- Zero runtime cost

---

## Internal Data Flow

### Download Flow

```
User calls storage.download(handle)
         │
         ▼
┌─────────────────────────────────────┐
│ MTP layer: Storage::download()      │
│ - Validates handle                  │
│ - Creates DownloadStream            │
└──────────────────┬──────────────────┘
                   │
                   ▼
┌─────────────────────────────────────┐
│ PTP layer: Session::get_object()    │
│ - Builds GetObject command          │
│ - Sends via transport               │
│ - Receives data containers          │
│ - Yields chunks to stream           │
└──────────────────┬──────────────────┘
                   │
                   ▼
┌─────────────────────────────────────┐
│ Transport: receive_bulk()           │
│ - USB bulk IN transfer              │
│ - Returns raw bytes                 │
└─────────────────────────────────────┘
```

### Upload Flow

```
User calls storage.upload(parent, info, data_stream)
         │
         ▼
┌─────────────────────────────────────┐
│ MTP layer: Storage::upload()        │
│ - Converts NewObjectInfo → ObjectInfo│
└──────────────────┬──────────────────┘
                   │
                   ▼
┌─────────────────────────────────────┐
│ PTP layer: Session::send_object_info│
│ - Serializes ObjectInfo             │
│ - Sends command + data phase        │
│ - Gets assigned handle              │
└──────────────────┬──────────────────┘
                   │
                   ▼
┌─────────────────────────────────────┐
│ PTP layer: Session::send_object()   │
│ - Streams data from user stream     │
│ - Chunks into USB packets           │
│ - Sends via transport               │
└──────────────────┬──────────────────┘
                   │
                   ▼
┌─────────────────────────────────────┐
│ Transport: send_bulk()              │
│ - USB bulk OUT transfer             │
└─────────────────────────────────────┘
```

---

## Concurrency Model

### Session Serialization

MTP transactions are synchronous at the protocol level - only one operation can be in progress at a time. The
`PtpSession` ensures this:

```rust
struct PtpSession {
    transport: Arc<NusbTransport>,
    transaction_id: AtomicU32,
    lock: AsyncMutex<()>,  // Ensures one operation at a time
}

impl PtpSession {
    async fn execute(&self, op: OperationCode, params: &[u32]) -> Result<Response> {
        let _guard = self.lock.lock().await;  // Serialize operations
        // ... execute operation
    }
}
```

### Event Handling

Events are received on a separate USB interrupt endpoint and can arrive anytime:

```rust
impl PtpSession {
    fn events(&self) -> impl Stream<Item=DeviceEvent> {
        // Spawns background task to poll interrupt endpoint
        // Uses bounded broadcast channel (capacity 100)
        // If buffer full, oldest events dropped
        // Returns channel receiver as stream
    }
}
```

**Multiple subscribers**: Each call to `events()` returns an independent stream.
All subscribers receive all events (broadcast pattern).

**Backpressure**: Events are buffered up to 100 entries. If no consumer is keeping
up, oldest events are dropped to prevent unbounded memory growth.

### Thread Safety

- `MtpDevice` is `Send + Sync`
- Multiple `Storage` references can exist (they hold `Arc<Device>`)
- Concurrent operations are serialized internally
- Event stream can be polled from any task

### Cancellation and Cleanup

**Download cancellation**: When a `DownloadStream` is dropped mid-transfer, the
`Drop` implementation drains remaining data containers from the USB to maintain
protocol consistency. This happens synchronously and may block briefly.

**Upload cancellation**: If an upload future is dropped after `SendObjectInfo`
succeeds but before `SendObject` completes, a partial/empty object may remain
on the device. The protocol has no abort mechanism. Callers should track the
handle and delete incomplete objects if needed.

**Session cleanup**: When `MtpDevice` is dropped, `CloseSession` is sent
automatically. No USB reset is performed.

---

## Error Handling Strategy

### Error Propagation

Errors bubble up through layers with context:

```
Transport error (nusb::Error)
         │
         ▼
    Error::Usb(...)
         │
         ▼
Protocol error (bad response code)
         │
         ▼
    Error::Protocol { code, operation }
```

### Retryable Errors

Some errors are transient:

```rust
impl Error {
    pub fn is_retryable(&self) -> bool {
        matches!(self,
            Error::Protocol { code: ResponseCode::DeviceBusy, .. } |
            Error::Timeout
        )
    }
}
```

### Error Context

Protocol errors include the operation that caused them:

```rust
Error::Protocol {
code: ResponseCode::InvalidObjectHandle,
operation: OperationCode::GetObject,
}
```

---

## Testing Strategy

### Unit Tests (no USB)

```
src/ptp/pack.rs        → test serialization with known byte sequences
src/ptp/container.rs   → test container building/parsing
src/ptp/types.rs       → test dataset serialization
```

### Protocol Tests (mock transport)

```rust
#[test]
async fn test_open_session() {
    let mut mock = MockTransport::new();
    mock.expect_send(/* OpenSession command */);
    mock.queue_response(/* OK response */);

    let session = PtpSession::new(mock);
    session.open(1).await.unwrap();
}
```

### Integration Tests (real device)

```rust
#[test]
#[ignore]  // Only run with --ignored
async fn test_real_device() {
    let device = MtpDevice::open_first().await.unwrap();
    let storages = device.storages().await.unwrap();
    assert!(!storages.is_empty());
}
```

---

## File Ownership

| File                | Responsibility                                |
|---------------------|-----------------------------------------------|
| `lib.rs`            | Crate root, public re-exports                 |
| `error.rs`          | All error types                               |
| `ptp/codes.rs`      | Operation, response, event, format code enums |
| `ptp/pack.rs`       | Binary serialization primitives               |
| `ptp/container.rs`  | USB container format                          |
| `ptp/types.rs`      | DeviceInfo, StorageInfo, ObjectInfo structs   |
| `ptp/session.rs`    | Transaction management, operation execution   |
| `ptp/device.rs`     | PtpDevice public API                          |
| `mtp/device.rs`     | MtpDevice, MtpDeviceBuilder                   |
| `mtp/storage.rs`    | Storage struct and methods                    |
| `mtp/object.rs`     | ObjectInfo, NewObjectInfo, ObjectFormat       |
| `mtp/event.rs`      | DeviceEvent enum, event stream                |
| `mtp/stream.rs`     | DownloadStream, upload helpers                |
| `transport/mod.rs`  | Transport trait                               |
| `transport/nusb.rs` | nusb implementation                           |
