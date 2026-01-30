# Rust Rewrite Feasibility Assessment for libmtp

## Executive Summary

Rewriting libmtp in Rust is **feasible** but requires significant effort, particularly around testing and compatibility
verification. The codebase has clean architecture that maps well to Rust, but lacks existing tests to verify
implementation correctness.

---

## Codebase Analysis

### Size and Complexity

| Component    | Lines of Code | Files |
|--------------|---------------|-------|
| Total        | ~47,000       | 51    |
| Core library | ~20,000       | 20    |
| Examples     | ~3,400        | 29    |
| Utilities    | ~22,400       | 2     |

- **122 public API functions** to implement
- **1,521 device entries** with vendor/product IDs and quirks
- **16+ device flags** for compatibility workarounds

### Architecture

The codebase has clean separation into layers:

1. **MTP Abstraction Layer** (`libmtp.c` - 9,475 lines)
    - High-level C API for MTP device management
    - Device detection and connection handling
    - File, track, playlist, album, and folder operations
    - Property management and metadata retrieval

2. **PTP Protocol Implementation** (~13,900 lines)
    - `ptp.c` (9,217 lines): Core protocol state machine
    - `ptp.h` (4,705 lines): Protocol structures and constants
    - `ptp-pack.c` (3,218 lines): Binary serialization/deserialization

3. **USB Transport Layer** (~6,800 lines)
    - `libusb1-glue.c`: libusb 1.0 backend (recommended)
    - `libusb-glue.c`: libusb 0.x legacy backend
    - `libopenusb1-glue.c`: OpenUSB alternative backend

4. **Specialized Features**
    - `mtpz.c`: Encrypted MTP support (Zune devices)
    - `playlist-spl.c`: Samsung playlist format
    - `unicode.c`: UTF-8/UTF-16 conversion

### External Dependencies

| Dependency | Purpose                    | Rust Equivalent         |
|------------|----------------------------|-------------------------|
| libusb-1.0 | USB communication          | `rusb` crate            |
| libiconv   | Character encoding         | `encoding_rs` crate     |
| libgcrypt  | MTPZ encryption (optional) | `ring` or `rust-crypto` |

---

## Testing Infrastructure

### Current State: Virtually No Tests

- No unit test suite
- No integration tests
- No test framework
- CI (Travis) only verifies compilation with gcc/clang
- Example programs serve as informal functional tests
- Device compatibility verified through real-world usage only

### Implications

- **Positive**: No legacy test patterns to maintain or port
- **Negative**: No safety net to verify compatibility with original implementation

---

## Key Challenges for Rust Rewrite

### 1. Binary Protocol Handling

The PTP/MTP protocol requires extensive binary packing/unpacking with specific byte ordering and alignment. This is
error-prone and needs thorough testing.

### 2. Device Quirks Database

1,521 devices with specific workarounds encoded in `music-players.h`. Device flags include:

- `DEVICE_FLAG_BROKEN_MTPGETOBJPROPLIST_ALL`
- `DEVICE_FLAG_NO_RELEASE_INTERFACE`
- `DEVICE_FLAG_PLAYLIST_SPL_V1/V2`
- `DEVICE_FLAG_ONLY_7BIT_FILENAMES`

### 3. Multiple USB Backends

Supporting libusb 1.0, legacy 0.x, and OpenUSB for maximum compatibility.

### 4. Callback-Heavy C API

The current API uses callbacks extensively for:

- Transfer progress reporting
- Custom data I/O handlers
- Event notifications

These need idiomatic Rust redesign (traits, async, channels).

### 5. Platform-Specific Code

- Linux: udev integration
- macOS: IOKit
- Windows: MinGW support

---

## Recommendations for Ensuring Compatibility

### 1. Protocol-Level Test Suite

Create tests against real MTP device captures:

- Capture USB traffic from real devices using Wireshark/usbmon
- Create test fixtures with known request/response pairs
- Verify Rust implementation produces identical bytes

```rust
#[test]
fn test_get_device_info_request() {
    let request = GetDeviceInfoRequest::new();
    let bytes = request.serialize();
    assert_eq!(bytes, include_bytes!("fixtures/get_device_info_request.bin"));
}
```

### 2. C-Compatible FFI Layer

Build a compatibility shim exposing the same C API:

```rust
#[no_mangle]
pub extern "C" fn LIBMTP_Get_First_Device() -> *mut LIBMTP_mtpdevice_t {
    // Rust implementation
}
```

This allows running existing example programs against both implementations.

### 3. Property-Based Testing

Use `proptest` or `quickcheck` to generate random:

- File metadata structures
- Object property values
- Protocol messages

Verify round-trip serialization matches C implementation behavior.

### 4. Device Simulation

Build a mock MTP device using Linux USB gadget framework:

- Responds to all MTP operations
- Validates protocol conformance
- Tests edge cases that quirks flags address

### 5. Differential Testing

Run both C and Rust implementations side-by-side:

- Send identical commands to real devices
- Compare responses byte-for-byte
- Log any divergences for investigation

---

## Recommended Implementation Strategy

### Phase 1: PTP Protocol Layer

Start with the most self-contained component:

- Implement all PTP data structures
- Build serialization/deserialization
- Comprehensive unit tests with captured data
- No external dependencies needed

### Phase 2: USB Transport

Layer on device communication:

- Use `rusb` crate for libusb bindings
- Implement device detection and enumeration
- Bulk and interrupt transfer handling

### Phase 3: MTP Abstraction

Build the high-level API:

- Design idiomatic Rust API (not just C port)
- Implement file/track/playlist operations
- Property and metadata handling

### Phase 4: Device Quirks

Port the device database:

- Convert `music-players.h` to Rust data structure
- Implement quirk detection and workarounds
- Test against known problematic devices

### Phase 5: Optional C FFI

For drop-in replacement compatibility:

- Expose C-compatible function signatures
- Match memory layout of public structures
- Enable gradual migration for existing users

---

## Feasibility Verdict

| Aspect                  | Rating          | Notes                                       |
|-------------------------|-----------------|---------------------------------------------|
| Technical feasibility   | **High**        | Clean architecture maps well to Rust        |
| Effort estimate         | **Medium-High** | ~47K lines to understand and reimplement    |
| Testing complexity      | **High**        | Must build test infrastructure from scratch |
| Compatibility assurance | **Medium**      | Achievable with proper test strategy        |
| Maintenance benefit     | **High**        | Memory safety, modern tooling, better API   |

### Overall Assessment: **Feasible with Significant Investment**

The rewrite is technically straightforward due to the well-structured C codebase. The main challenge is building
confidence in compatibility without existing tests. A phased approach starting with the protocol layer, combined with
aggressive testing against real device captures, provides the best path forward.

### Estimated Scope

- **Core protocol + USB**: 3-6 months for experienced Rust developer
- **Full feature parity**: 6-12 months including device quirks
- **Test infrastructure**: Ongoing effort, critical for success

### Risk Mitigation

1. Start with protocol layer (lowest risk, highest testability)
2. Capture real device traffic early for test fixtures
3. Maintain C FFI option for gradual adoption
4. Engage with libmtp community for edge case knowledge

---

## References

- [USB MTP 1.0 Specification](https://www.usb.org/developers/devclass_docs/MTP_1.0.zip)
- [rusb crate](https://crates.io/crates/rusb) - Rust libusb bindings
- [libmtp source](https://github.com/libmtp/libmtp)
- PTP protocol derived from libgphoto2
