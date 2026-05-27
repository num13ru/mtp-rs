# mtp-rs

Pure-Rust MTP/PTP library with no C dependencies. Two-layer API: `mtp::` for high-level file transfer, `ptp::` for low-level protocol access (cameras). Zero FFI - no libmtp, no libusb, just async Rust on `nusb`.

This repo is a Cargo workspace. The library lives in `crates/mtp-rs/` and is published as `mtp-rs`. A companion CLI binary lives in `crates/mtp-rs-cli/` and is published as `mtp-rs-cli` (the installed binary is named `mtp-rs`).

## Quick commands

| Command                                | Description                                          |
|----------------------------------------|------------------------------------------------------|
| `just`                                 | Run all checks: format, lint, test, doc              |
| `just fix`                             | Auto-fix formatting and clippy warnings              |
| `just check-all`                       | Include MSRV check, security audit, license check    |
| `just release-dry`                     | `cargo publish --dry-run` for both crates            |
| `cargo test --workspace --all-features`| Run with proptest fuzzing across the workspace       |

## Project structure

```
crates/
  mtp-rs/                    # Library (crates.io: mtp-rs)
    src/
      mtp/                   # High-level API (MtpDevice, Storage)
      ptp/                   # Low-level protocol (PtpDevice, PtpSession)
        codes.rs             # OperationCode, ResponseCode, EventCode
      transport/             # USB abstraction (Transport trait, nusb, mock, virtual_device)
    tests/integration.rs     # Real-device tests
    examples/                # list_and_download, ptp_diagnose, fuji_capture, etc.
  mtp-rs-cli/                # CLI binary (crates.io: mtp-rs-cli, binary: mtp-rs)
    src/
      main.rs                # Entry point
      cli/                   # Subcommand dispatch, args, error mapping, paths
    tests/cli.rs             # Cross-process tests via the built binary + virtual device
    docs/cli.md              # Full command reference
benchmarks/
  mtp-rs-vs-libmtp/          # Throughput vs libmtp comparison (not published)
docs/                        # Protocol, architecture, debugging, release process
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

**Key types:** `ObjectHandle`, `StorageId` (newtypes), `AccessCapability`, `OperationCode`, `UsbSpeed` (negotiated USB link speed surfaced on `MtpDeviceInfo::speed` / `UsbDeviceInfo::speed`; both info structs are `#[non_exhaustive]`, so add fields freely without breaking consumers).

## Known device quirks

- **Android**: `ObjectHandle::ALL` recursive listing broken; library auto-detects via `"android.com"` in vendor extension
- **Android**: Uploads to the storage root are rejected with `InvalidObjectHandle`. Upload into an existing folder (for example, `Download`) instead.
- **Fujifilm cameras**: Report `AccessCapability::ReadWrite` but return `StoreReadOnly` on writes. Advertised ops lie.
- **Samsung**: Returns `InvalidObjectHandle` for root listing; needs recursive traversal with filtering

## Testing

- **Unit**: `cargo test --workspace` (uses mock transport)
- **Virtual device**: `cargo test -p mtp-rs --features virtual-device` (full protocol tests against local filesystem)
- **Integration**: `cargo test -p mtp-rs --test integration -- --ignored --nocapture` (needs device). Destructive tests pick a writable root folder from a priority list (Android `Download`, Garmin `Music`, Kindle `documents`, etc.); set `MTP_TEST_FOLDER=Name` to override. See `crates/mtp-rs/tests/integration.rs` header for full details.
- **CLI**: `cargo test -p mtp-rs-cli --features virtual-device` (runs the built binary against a virtual device)
- **Property**: `cargo test --workspace --all-features` (proptest fuzzing)

## Design principles

- **Pure Rust**: No C/FFI, no `-sys` crates
- **Runtime-agnostic**: `futures` traits only, no tokio/async-std dependency
- **Stream-based**: Downloads and uploads stream via `Stream<Item = Chunk>` for memory efficiency
- **Safe cancellation**: Mid-stream downloads can be cancelled via USB SIC class cancel
- **Type-safe handles**: Newtypes prevent ID mixups

## Cooperative cancellation for list/delete ops

`Storage::list_objects_with_cancel`, `list_objects_stream_with_cancel`, and
`delete_with_cancel` take an `Option<&CancelToken>`. The token is
`Arc<AtomicBool>`-backed, cheap to clone, and one-way (no reset; make a fresh
token per logical op). When set, `ObjectListing::next` checks before issuing
each `GetObjectInfo` USB roundtrip and bails with `Err(Error::Cancelled)`.

If you already have an `Arc<AtomicBool>` driving cancellation on the consumer
side (a write-operation intent flag, a shared abort signal, anything), use
`CancelToken::from_arc(arc)` to wrap it without a second polling task. The
constructor shares the atomic, so flipping the consumer-side bool also flips
the token; `Default::default()` builds a fresh one from scratch.

For per-handle list/delete this is sufficient and safer than mid-USB-transaction
cancel: each `GetObjectInfo` and `DeleteObject` roundtrip completes in
milliseconds, so there's no half-finished transfer to drain. The CancelToken
short-circuits the per-handle for-loop, which is where 1k-entry Android folder
listings actually spend their 15+ seconds.

Streaming downloads keep using the SIC class-cancel path (see below); that's a
different mechanism for a different problem (one big bulk-IN to drain).

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

## Streaming uploads (USB bulk transfer details)

Uploads use `Transport::send_bulk_streaming()` to avoid buffering the entire
file in RAM. Key implementation notes:

- PTP data containers can span multiple USB bulk transfers. The device
  detects end-of-data via a short packet (< max packet size) or a
  zero-length packet (ZLP) when the total is a multiple of max packet size.
- Each `Endpoint::submit()` call is a separate USB transfer. The header
  (12 bytes) is prepended to the first chunk so the device sees the PTP
  container header in the first transfer (matching libmtp behavior).
- Data is batched into 256KB USB transfers using nusb's low-level
  `allocate/submit/wait_next_complete` API. `EndpointWrite` would be
  cleaner but requires ownership of the `Endpoint`, which lives behind
  a `Mutex` in `NusbTransport`.
- A ZLP must be sent after the final transfer if its size is a multiple
  of `max_packet_size`. Without this, Android devices hang waiting for
  more data (validated on Pixel 9 Pro XL).
- Mock and virtual transports use the default implementation which
  buffers everything and calls `send_bulk()`.
- See `NusbTransport::send_bulk_streaming()` for the full implementation.

## Receiving data containers (multi-transfer convention)

PTP data containers may span multiple USB bulk transfers on receive too: some
devices (Garmin Forerunner 955, observed) send the 12-byte container header in
one bulk transfer and the payload in a follow-up transfer. **Any new code path
that calls `receive_bulk()` and expects a `DataContainer` must accumulate
transfers until `bytes.len() >= total_length` (read from the first 4 bytes of
the header) before parsing.** See `PtpSession::execute_with_receive` and
`PtpDevice::get_device_info` for the canonical pattern. Skipping this loop
breaks GetDeviceInfo on spec-compliant devices that split.

## Test-time backing-dir drain (virtual-device only)

External test fixtures that delete and recreate files in a virtual device's backing dir hit the same race the watcher's pause/resume is designed to prevent: FS events from the writes can land *after* the rescan and resume, and the watcher then incorrectly emits removes for the freshly re-added objects. Old approach: pause, write, sleep ≥600 ms (macOS FSEvents worst-case), rescan, resume. Slow and brittle.

The current API supports an event-driven drain:

- `pause_watcher(serial)` returns a `WatcherGuard`. The pause is **refcounted** (`VirtualDeviceState::pause_count`), so multiple concurrent guards compose — the watcher only resumes when the last guard drops. Tests can drain in parallel without stepping on each other.
- While at least one guard is alive, every dropped FS event's canonical path is pushed into `VirtualDeviceState::dropped_paths` (a `VecDeque` capped at `DROPPED_PATHS_CAP = 1024`, oldest evicted past that).
- `dropped_paths_since_pause(serial) -> Vec<PathBuf>` is the **primary observation primitive**: returns a clone of the ring, oldest first.
- `was_path_dropped(serial, suffix) -> bool` is a thin convenience over the above for the sentinel-file drain pattern: write a uniquely-named file as the LAST fixture step (per-directory FS-event ordering on every supported `notify` backend means every earlier write to the same directory already arrived once you see the sentinel), then poll this until it returns `true`.
- `clear_dropped_paths(serial)` empties the ring; call after a successful drain so the buffer stays scoped to in-flight pauses.

**Why suffix-match, not exact path**: macOS canonicalizes `/tmp` → `/private/tmp`, the watcher canonicalizes again, and the backing-dir path may be relative. Suffix-match sidesteps the whole class of false negatives. Choose a unique enough suffix (UUID-bearing filename) so concurrent drains don't false-positive on each other.

**Composing your own pattern**: any test harness that doesn't fit sentinel-file (counting events under a subdir, declaring quiet when the count hasn't grown for N polls) should call `dropped_paths_since_pause` directly. `was_path_dropped` exists for the common case only.

Unit tests for the API live in `transport/virtual_device/registry.rs` (`pause_refcount_composes_across_concurrent_guards`, `dropped_paths_observation_round_trip`, `dropped_paths_ring_evicts_oldest_past_cap`, and the unknown-serial defensive paths).

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
- [docs/notes/community-threads.md](docs/notes/community-threads.md) — required reading before working on issues or PRs. Recap of every GitHub thread so far, known device quirks, and recurring contributors. Update after work that affects community-facing context.
- [MTP v1.1 Spec](https://github.com/vdavid/mtp-v1_1-spec-md)
