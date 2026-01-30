# Test Strategy for libmtp Rust Rewrite

This document outlines the testing strategy for rewriting libmtp in Rust while ensuring compatibility with the original
C implementation.

## Goals

1. **100% protocol compatibility** with the original libmtp
2. **No dependency on real MTP devices** for regular testing
3. **High confidence** in serialization/deserialization correctness
4. **Regression prevention** through comprehensive test coverage

## Architecture for Testability

The key architectural decision is **abstracting the USB transport layer** to enable mock injection:

```
┌─────────────────────────────────────────────────────────┐
│                    Public API                           │
│  (MtpDevice, FileOperations, TrackOperations, etc.)     │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                  MTP Protocol Layer                     │
│  (Session management, object operations, properties)    │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                  PTP Protocol Layer                     │
│  (Container building, transaction management)           │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│              Transport Trait (abstract)                 │
│  fn send_command(&self, container: &CommandContainer)   │
│  fn receive_data(&self) -> Result<DataContainer>        │
│  fn receive_response(&self) -> Result<ResponseContainer>│
└───────┬─────────────────────────────────────────┬───────┘
        │                                         │
┌───────▼───────┐                         ┌───────▼───────┐
│  NusbTransport │                         │  MockTransport │
│  (real USB)    │                         │  (for tests)   │
└────────────────┘                         └────────────────┘
```

## Test Layers

### Layer 1: Serialization Unit Tests

Test the binary protocol packing/unpacking in isolation. This covers the equivalent of `ptp-pack.c`.

**What to test:**

- Primitive types (u16, u32, u64) in little-endian format
- PTP string encoding (UTF-16LE with length prefix)
- Array encoding (count prefix + elements)
- Complex structures (ObjectInfo, DeviceInfo, StorageInfo)

**Example tests:**

```rust
#[test]
fn test_u32_little_endian() {
    assert_eq!(pack_u32(0x12345678), [0x78, 0x56, 0x34, 0x12]);
    assert_eq!(unpack_u32(&[0x78, 0x56, 0x34, 0x12]), 0x12345678);
}

#[test]
fn test_string_ascii() {
    // "Hi" → length=3 (includes null), then UTF-16LE chars + null
    let packed = pack_ptp_string("Hi");
    assert_eq!(packed, vec![
        0x03,             // length (2 chars + null terminator)
        0x48, 0x00,       // 'H'
        0x69, 0x00,       // 'i'
        0x00, 0x00,       // null terminator
    ]);
}

#[test]
fn test_u32_array() {
    let arr = vec![0x00010001u32, 0x00010002, 0xFFFFFFFF];
    let packed = pack_u32_array(&arr);
    assert_eq!(packed, vec![
        0x03, 0x00, 0x00, 0x00,  // count = 3
        0x01, 0x00, 0x01, 0x00,  // storage ID 1
        0x02, 0x00, 0x01, 0x00,  // storage ID 2
        0xFF, 0xFF, 0xFF, 0xFF,  // storage ID 3
    ]);
}

#[test]
fn test_string_roundtrip() {
    let original = "Test File 音楽.mp3";
    let packed = pack_ptp_string(original);
    let unpacked = unpack_ptp_string(&packed).unwrap();
    assert_eq!(unpacked, original);
}
```

### Layer 2: Container/Protocol Tests

Test PTP container building and parsing.

**What to test:**

- Command container assembly (1-5 parameters)
- Data container with payloads of various sizes
- Response container parsing
- Multi-packet handling for large transfers
- Transaction ID sequencing

**Example tests:**

```rust
#[test]
fn test_command_container_open_session() {
    let cmd = CommandContainer {
        code: OpCode::OpenSession,
        transaction_id: 1,
        params: vec![1], // session ID = 1
    };

    let bytes = cmd.to_bytes();

    assert_eq!(bytes, vec![
        0x10, 0x00, 0x00, 0x00,  // length = 16
        0x01, 0x00,              // type = Command
        0x02, 0x10,              // code = 0x1002 (OpenSession)
        0x01, 0x00, 0x00, 0x00,  // transaction_id = 1
        0x01, 0x00, 0x00, 0x00,  // param1 = 1 (session ID)
    ]);
}

#[test]
fn test_response_container_parse() {
    let bytes = vec![
        0x0C, 0x00, 0x00, 0x00,  // length = 12
        0x03, 0x00,              // type = Response
        0x01, 0x20,              // code = 0x2001 (OK)
        0x01, 0x00, 0x00, 0x00,  // transaction_id = 1
    ];

    let resp = ResponseContainer::from_bytes(&bytes).unwrap();

    assert_eq!(resp.code, ResponseCode::Ok);
    assert_eq!(resp.transaction_id, 1);
}
```

### Layer 3: Mock Transport for Protocol Tests

A mock USB transport that allows testing protocol logic without real hardware.

**Implementation:**

```rust
pub struct MockTransport {
    responses: VecDeque<Vec<u8>>,
    sent_commands: Vec<Vec<u8>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self {
            responses: VecDeque::new(),
            sent_commands: Vec::new(),
        }
    }

    pub fn queue_response(&mut self, data: Vec<u8>) {
        self.responses.push_back(data);
    }

    pub fn queue_ok(&mut self, transaction_id: u32) {
        self.queue_response(ResponseContainer {
            code: ResponseCode::Ok,
            transaction_id,
            params: vec![],
        }.to_bytes());
    }

    pub fn get_sent_commands(&self) -> &[Vec<u8>] {
        &self.sent_commands
    }
}

impl Transport for MockTransport {
    fn send(&mut self, data: &[u8]) -> Result<()> {
        self.sent_commands.push(data.to_vec());
        Ok(())
    }

    fn receive(&mut self) -> Result<Vec<u8>> {
        self.responses.pop_front()
            .ok_or(Error::NoMoreResponses)
    }
}
```

**Example tests:**

```rust
#[test]
fn test_open_session_protocol() {
    let mut transport = MockTransport::new();
    transport.queue_ok(1);

    let mut session = PtpSession::new(transport);
    session.open(1).unwrap();

    let sent = session.transport().get_sent_commands();
    assert_eq!(sent.len(), 1);

    let cmd = CommandContainer::from_bytes(&sent[0]).unwrap();
    assert_eq!(cmd.code, OpCode::OpenSession);
    assert_eq!(cmd.params, vec![1]);
}

#[test]
fn test_get_storage_ids() {
    let mut transport = MockTransport::new();

    // Queue data response (array of storage IDs)
    transport.queue_response(DataContainer {
        code: OpCode::GetStorageIds,
        transaction_id: 2,
        payload: pack_u32_array(&[0x00010001, 0x00010002]),
    }.to_bytes());
    transport.queue_ok(2);

    let mut session = PtpSession::with_mock(transport);
    session.transaction_id = 2;

    let storage_ids = session.get_storage_ids().unwrap();
    assert_eq!(storage_ids, vec![0x00010001, 0x00010002]);
}
```

### Layer 4: Capture Replay Tests

Replay captured USB traffic from real devices for realistic testing.

**Transport implementation:**

```rust
pub struct CaptureReplayTransport {
    captures: Vec<CapturedExchange>,
    current: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CapturedExchange {
    pub description: String,
    pub expected_command: String,  // hex-encoded
    pub responses: Vec<String>,    // hex-encoded
}

impl CaptureReplayTransport {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let captures: Vec<CapturedExchange> = serde_json::from_str(&content)?;
        Ok(Self { captures, current: 0 })
    }
}
```

**Capture file format (JSON):**

```json
{
  "device": "Google Pixel 6",
  "android_version": "14",
  "captured_at": "2024-01-15T10:30:00Z",
  "exchanges": [
    {
      "description": "OpenSession",
      "command": "10000000010002100100000001000000",
      "responses": [
        "0c00000003000120010000"
      ]
    },
    {
      "description": "GetStorageIDs",
      "command": "0c0000000100041002000000",
      "responses": [
        "180000000200041002000000020000000100010002000100",
        "0c00000003000120020000"
      ]
    }
  ]
}
```

**Example test:**

```rust
#[test]
fn test_full_session_with_pixel6_capture() {
    let transport = CaptureReplayTransport::from_file(
        "fixtures/captures/pixel6_list_files.json"
    ).unwrap();

    let mut device = MtpDevice::new(transport);
    device.open_session().unwrap();

    let files = device.get_file_listing(STORAGE_ALL, None).unwrap();

    // Verify against known state when capture was made
    assert!(files.iter().any(|f| f.filename == "DCIM"));
    assert!(files.iter().any(|f| f.filename == "Download"));
}
```

### Layer 5: Property-Based Testing

Use `proptest` to automatically find edge cases.

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn string_roundtrip(s in "\\PC{0,255}") {
        let packed = pack_ptp_string(&s);
        let unpacked = unpack_ptp_string(&packed).unwrap();
        prop_assert_eq!(s, unpacked);
    }

    #[test]
    fn u32_array_roundtrip(arr in prop::collection::vec(any::<u32>(), 0..1000)) {
        let packed = pack_u32_array(&arr);
        let unpacked = unpack_u32_array(&packed).unwrap();
        prop_assert_eq!(arr, unpacked);
    }

    #[test]
    fn object_info_roundtrip(
        storage_id in any::<u32>(),
        format in 0u16..0x4000,
        size in any::<u32>(),
        filename in "[a-zA-Z0-9_]{1,64}\\.(mp3|jpg|txt)",
    ) {
        let info = ObjectInfo {
            storage_id,
            object_format: ObjectFormat::from_code(format),
            compressed_size: size,
            filename,
            ..Default::default()
        };

        let packed = info.pack();
        let unpacked = ObjectInfo::unpack(&packed).unwrap();

        prop_assert_eq!(info.storage_id, unpacked.storage_id);
        prop_assert_eq!(info.filename, unpacked.filename);
    }

    #[test]
    fn command_container_valid(
        code in 0x1001u16..0x1030,
        transaction_id in 1u32..u32::MAX,
        num_params in 0usize..=5,
    ) {
        let params: Vec<u32> = (0..num_params).map(|i| i as u32).collect();

        let container = CommandContainer {
            code: OpCode::from_code(code),
            transaction_id,
            params,
        };

        let bytes = container.to_bytes();

        // Verify length field is correct
        let length = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        prop_assert_eq!(length as usize, bytes.len());

        // Verify type field
        let container_type = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        prop_assert_eq!(container_type, 0x0001); // Command
    }
}
```

### Layer 6: Differential Testing Against C Library

Build both libraries and compare outputs for the same inputs.

```rust
#[cfg(test)]
mod differential_tests {
    use libmtp_sys::*; // FFI bindings to C library

    #[test]
    fn compare_object_info_packing() {
        let test_cases = vec![
            ("simple.mp3", 1000, 0x00010001),
            ("文件.txt", 500, 0x00010002),
            ("a".repeat(255), u32::MAX, 0xFFFFFFFF),
        ];

        for (filename, size, storage_id) in test_cases {
            // Pack with Rust implementation
            let rust_info = ObjectInfo {
                filename: filename.clone(),
                compressed_size: size,
                storage_id,
                ..Default::default()
            };
            let rust_bytes = rust_info.pack();

            // Pack with C implementation via FFI
            let c_bytes = unsafe {
                let c_info = create_c_object_info(&filename, size, storage_id);
                pack_object_info_c(c_info)
            };

            assert_eq!(rust_bytes, c_bytes,
                       "Mismatch for filename={}, size={}", filename, size);
        }
    }
}
```

### Layer 7: USB Gadget Simulator (Optional)

For comprehensive end-to-end testing on Linux without a real device.

```rust
/// Software MTP device using Linux USB gadget
pub struct MtpGadgetSimulator {
    storage: HashMap<u32, SimulatedStorage>,
    objects: HashMap<u32, SimulatedObject>,
}

impl MtpGadgetSimulator {
    /// Configure as specific device for quirk testing
    pub fn emulate_device(device_flags: DeviceFlags) -> Self { ... }

    /// Add files to the simulated storage
    pub fn add_file(&mut self, path: &str, content: &[u8]) { ... }
}
```

This requires Linux with ConfigFS and USB gadget support. More complex but provides true end-to-end testing.

## Test File Structure

```
tests/
├── fixtures/
│   ├── captures/
│   │   ├── pixel6_list_files.json
│   │   ├── samsung_s23_transfer.json
│   │   └── quirky_device_edge_cases.json
│   ├── device_info/
│   │   ├── pixel6.bin
│   │   ├── samsung_s23.bin
│   │   └── generic_android.bin
│   └── object_info/
│       ├── mp3_file.bin
│       ├── jpeg_image.bin
│       └── folder.bin
├── unit/
│   ├── pack_primitives.rs
│   ├── pack_strings.rs
│   ├── pack_arrays.rs
│   ├── pack_structures.rs
│   └── containers.rs
├── protocol/
│   ├── session_management.rs
│   ├── storage_operations.rs
│   ├── object_operations.rs
│   └── property_operations.rs
├── integration/
│   ├── capture_replay.rs
│   └── differential.rs
└── proptest/
    ├── serialization.rs
    └── protocol_fuzzing.rs
```

## Capturing Test Data

To create test fixtures, capture USB traffic once from a real device:

### Using usbmon (Linux)

```bash
# Load usbmon module
sudo modprobe usbmon

# Find your device's bus number
lsusb | grep -i android

# Start capture (replace 2 with your bus number)
sudo cat /sys/kernel/debug/usb/usbmon/2u > capture.txt &

# Run a simple MTP operation
mtp-detect  # or mtp-files

# Stop capture (Ctrl+C)
# Convert to test fixtures using a parsing script
```

### Using Wireshark

1. Start Wireshark with USB capture
2. Filter for your device's USB address
3. Perform MTP operations
4. Export as JSON or PCAP
5. Convert to test fixture format

This is **read-only observation** - no risk to the device. Capture once to get realistic test data, then never need the
device again for testing.

## Test Coverage Summary

| Test Layer               | Coverage                 | Real Device Needed |
|--------------------------|--------------------------|--------------------|
| Serialization unit tests | Byte-level correctness   | No                 |
| Container tests          | Protocol message format  | No                 |
| Mock transport tests     | Protocol flow logic      | No                 |
| Capture replay tests     | Real-world compatibility | Once (to capture)  |
| Property-based tests     | Edge cases, fuzzing      | No                 |
| Differential tests       | C library compatibility  | No (just FFI)      |
| USB gadget simulator     | End-to-end               | No (Linux only)    |

## Key Protocol Details

### USB Container Structure

```
Offset  Type      Size   Field
0       uint32_t  4      length     (total packet size including header)
4       uint16_t  2      type       (1=Command, 2=Data, 3=Response, 4=Event)
6       uint16_t  2      code       (operation/response code)
8       uint32_t  4      trans_id   (transaction ID)
12      [payload] var    data       (parameters or payload)
```

### Container Types

- `0x0001` - Command Container
- `0x0002` - Data Container
- `0x0003` - Response Container
- `0x0004` - Event Container

### Common Operation Codes

- `0x1001` - GetDeviceInfo
- `0x1002` - OpenSession
- `0x1003` - CloseSession
- `0x1004` - GetStorageIDs
- `0x1005` - GetStorageInfo
- `0x1007` - GetObjectHandles
- `0x1008` - GetObjectInfo
- `0x1009` - GetObject
- `0x100B` - DeleteObject
- `0x100C` - SendObjectInfo
- `0x100D` - SendObject

### Common Response Codes

- `0x2001` - OK
- `0x2002` - GeneralError
- `0x2005` - OperationNotSupported
- `0x2009` - InvalidObjectHandle

## Dependencies

Recommended Rust crates for testing:

- `nusb` - Pure Rust USB library (production)
- `proptest` - Property-based testing
- `serde` / `serde_json` - Capture file parsing
- `hex` - Hex encoding/decoding for fixtures

For differential testing:

- `bindgen` - Generate FFI bindings to original libmtp
- `cc` - Build C library for comparison

## Implementation Order

1. **Start with serialization** - Implement and test `ptp-pack` equivalent
2. **Add container handling** - Build and parse protocol containers
3. **Implement mock transport** - Enable protocol testing
4. **Capture real traffic** - Create fixture files from one device session
5. **Build protocol layer** - Session, storage, object operations
6. **Add property tests** - Find edge cases automatically
7. **Optional: differential tests** - Compare against C library
8. **Optional: USB gadget** - Full end-to-end on Linux
