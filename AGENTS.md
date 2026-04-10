# mtp-rs

Pure-Rust MTP/PTP library with no C dependencies. Two-layer API: `mtp::` for high-level file transfer, `ptp::` for low-level protocol access (cameras). Zero FFI - no libmtp, no libusb, just async Rust on `nusb`.

## Quick commands

| Command                     | Description                                          |
|-----------------------------|------------------------------------------------------|
| `just`                      | Run all checks: format, lint, test, doc              |
| `just fix`                  | Auto-fix formatting and clippy warnings              |
| `just check-all`            | Include security audit and license check             |
| `cargo test --all-features` | Run with proptest for fuzzing                        |

## Project structure

```
src/
  mtp/         # High-level API (MtpDevice, Storage)
  ptp/         # Low-level protocol (PtpDevice, PtpSession)
    types/     # DeviceInfo, StorageInfo, ObjectInfo, AccessCapability
    codes.rs   # OperationCode, ResponseCode, EventCode
  transport/   # USB abstraction (Transport trait, nusb impl, mock, virtual_device)
examples/      # list_and_download, ptp_diagnose, fuji_capture, fuji_rw_check
```

## Architecture

```
mtp:: (MtpDevice, Storage)    <-- Android/media devices
  |
ptp:: (PtpSession)            <-- Cameras, protocol work
  |
transport:: (Transport trait)
  |
nusb (USB)  or  VirtualTransport (filesystem, feature = "virtual-device")
```

**Entry points:** `MtpDevice::open_first()`, `PtpDevice::open_first()`, `NusbTransport::list_mtp_devices()`, `MtpDeviceBuilder::open_virtual()` (feature-gated)

**Key types:** `ObjectHandle`, `StorageId` (newtypes), `AccessCapability`, `OperationCode`

## Known device quirks

- **Android**: `ObjectHandle::ALL` recursive listing broken; library auto-detects via `"android.com"` in vendor extension
- **Fujifilm cameras**: Report `AccessCapability::ReadWrite` but return `StoreReadOnly` on writes. Advertised ops lie.
- **Samsung**: Returns `InvalidObjectHandle` for root listing; needs recursive traversal with filtering

## Testing

- **Unit**: `cargo test` (uses mock transport)
- **Virtual device**: `cargo test --features virtual-device` (full protocol tests against local filesystem)
- **Integration**: `cargo test --test integration -- --ignored --nocapture` (needs device)
- **Property**: `cargo test --all-features` (proptest fuzzing)

## Design principles

- **Pure Rust**: No C/FFI, no `-sys` crates
- **Runtime-agnostic**: `futures` traits only, no tokio/async-std dependency
- **Stream-based**: Downloads return `Stream<Item = Chunk>` for memory efficiency
- **Safe cancellation**: Mid-stream downloads can be cancelled via USB SIC class cancel
- **Type-safe handles**: Newtypes prevent ID mixups

## Transfer cancellation

Mid-stream download cancellation uses the USB Still Image Class (SIC) cancel
mechanism: a CLASS_CANCEL control request (bRequest=0x64) followed by draining
the bulk IN and interrupt pipes. This approach was validated against libmtp's
`ptp_read_cancel_func` (Florent Viard, 2017). Key implementation notes:

- The drain must start **immediately** after CLASS_CANCEL — any delay (like
  polling GET_DEVICE_STATUS, which Android doesn't support) allows the device
  to enter an unrecoverable state.
- The drain uses maxpacket-sized reads with a 300ms idle timeout (matching
  libmtp and Windows behavior).
- The interrupt pipe must also be drained — some devices (GoPro) freeze if
  the CancelTransaction event is left unread.
- See `NusbTransport::cancel_transfer()` for the full implementation with
  detailed comments.

## Things to avoid

- C dependencies (libusb, libmtp, `-sys` crates)
- Device quirks database (understand issues first)
- MTPZ, vendor extensions, playlist/metadata sync
- Legacy workarounds (pre-Android 5.0)
- Runtime dependencies (use `futures` traits)

## Code style

Run `just check` before committing. `cargo fmt`, `cargo clippy -D warnings`, tests for new functionality, doc comments for public APIs.

## References

- [docs/architecture.md](docs/architecture.md), [docs/protocol.md](docs/protocol.md), [docs/debugging.md](docs/debugging.md)
- [docs/releasing.md](docs/releasing.md) — how to publish a new version to crates.io
- [MTP v1.1 Spec](https://github.com/vdavid/mtp-v1_1-spec-md)
