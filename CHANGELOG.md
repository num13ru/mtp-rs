# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.13.2] - 2026-04-27

### Fixed

- **Root listing is now fast on Kindle and other non-Android MTP devices.** `Storage::list_objects_stream(None)` previously took the slow `parent=0` path on any device that didn't advertise `"android.com"` in its vendor extension, which made root-level listings very slow on devices that return every object on the storage for `parent=0` (for example, Kindle Paperwhite 12th gen returned 2541 handles instead of 23 root-level items). The fast `parent=0xFFFFFFFF` path is now tried first for all devices, falling back to `parent=0` only when the device rejects it with an error. An empty `Ok(_)` from the fast path is treated as a legit empty storage, not a fallback trigger. Reported and fixed by [@num13ru](https://github.com/num13ru) in [#9](https://github.com/vdavid/mtp-rs/pull/9), closes [#8](https://github.com/vdavid/mtp-rs/issues/8).

### Changed

- The `is_android()` gate inside `Storage::list_objects_stream` is gone. The unified fast-path/fallback logic handles Android, Kindle, Samsung, and Fuji quirks without vendor-specific detection. The `is_android()` check inside `list_objects_recursive_auto` remains: it gates a different workaround.

## [0.13.1] - 2026-04-17

### Fixed

- **`Storage::get_object_info()` and `Storage::list_objects()` now return the real u64 size for files larger than 4 GB.** The standard `ObjectInfo` dataset encodes size as a u32 which saturates at `u32::MAX`; the new logic auto-resolves the full size via `GetObjectPropValue(ObjectSize)` when saturation is detected. Falls back to the saturated value on devices that don't support the follow-up op.

### Added

- **`PtpSession::get_object_info_full()`**: Low-level method that fetches ObjectInfo and resolves the u64 size when saturated.
- 5 new unit tests covering saturation detection, fallback behavior, and the edge case where a file's real size happens to equal `u32::MAX`.
- Virtual-device integration test that creates a 5 GB sparse file and verifies size resolution end-to-end.

### Changed

- Doc comment on `ObjectInfo::size` updated to reflect the new auto-resolution behavior of high-level APIs.

## [0.13.0] - 2026-04-17

### Added

- **`Storage::download_partial_64()`** and **`PtpSession::get_partial_object_64()`**: Byte-range reads with 64-bit offsets using the Android/MTP `GetPartialObject64` extension (0x95C1). Enables partial reads beyond the 4 GB boundary for large files (videos, archives, etc.). Tested end-to-end on a Pixel 9 Pro XL with an 8 GB file.
- **`OperationCode::GetPartialObject64`** variant
- Virtual device supports `GetPartialObject64` and advertises it in `operations_supported`
- New example `test_partial_download_64.rs` for real-device verification
- 3 new unit tests covering byte-range reads and 64-bit offset correctness

### Changed

- Documented the 4 GB offset limitation on `download_partial()` / `get_partial_object()` and cross-linked to the new 64-bit variants

## [0.12.0] - 2026-04-16

### Added

- **`Transport::send_bulk_streaming()`**: New trait method that sends data as a continuous USB transfer from a stream of chunks, with proper ZLP termination. Default implementation buffers and calls `send_bulk()`. `NusbTransport` streams in 256KB USB transfers using nusb's low-level endpoint API.

### Changed

- **Breaking:** `Storage::upload()` and `upload_with_progress()` now require `Send` on the stream type parameter
- **Breaking:** `Transport` trait has a new `send_bulk_streaming()` method (with a default implementation, so most custom impls don't need changes)
- **Breaking:** `PtpSession::execute_with_send_stream()` and `send_object_stream()` now require `Send` on the stream type parameter
- Uploads stream data directly to USB instead of buffering the entire file in memory. Peak memory during upload drops from O(file_size) to O(256KB).

## [0.11.1] - 2026-04-15

### Changed

- **Streaming uploads:** `Storage::upload()` and `upload_with_progress()` now stream data directly to USB via `send_object_stream` instead of buffering the entire file in memory. Peak memory during upload drops from O(file_size) to O(chunk_size). The API is unchanged.

## [0.11.0] - 2026-04-10

### Added

- **Safe mid-stream download cancellation:** `FileDownload::cancel(idle_timeout)` and `ReceiveStream::cancel(idle_timeout)` safely abort in-progress downloads using the USB Still Image Class cancel mechanism, leaving the session healthy for subsequent operations
- **`Transport::cancel_transfer()`** trait method with implementations for `NusbTransport`, `MockTransport`, and `VirtualTransport`
- **`DEFAULT_CANCEL_TIMEOUT`** (300ms) constant for the recommended cancel drain timeout
- **`EventCode::CancelTransaction`** variant (0x4001) in the event code enum
- **`EventContainer::to_bytes()`** serialization method (completes the `from_bytes`/`to_bytes` pair)
- `#[must_use]` on `ReceiveStream` and `FileDownload` — compiler warns if dropped without consuming or cancelling
- `debug_assert` in `ReceiveStream::Drop` catches accidental mid-stream drops during development

### Fixed

- `collect_with_progress` now properly cancels the USB transfer when the progress callback returns `ControlFlow::Break`, instead of just dropping the stream (which corrupted the session)

### Changed

- **Breaking:** `Transport` trait now requires `cancel_transfer()` — custom implementations must add this method
- `NusbTransport` now stores the USB `Interface` and interface number (needed for SIC cancel control transfers)

## [0.10.0] - 2026-04-09

### Added

- **Public low-level PTP execution primitives:** `PtpSession::execute()`, `execute_with_receive()`, and `execute_with_send()` are now public, enabling vendor-specific and non-standard MTP operations without forking the crate
- **`MtpDevice::session()`** accessor to reach the underlying `PtpSession` from the high-level API
- **Split header/data send mode:** `PtpSession::set_split_header_data()` / `is_split_header_data()` for devices that require the 12-byte PTP container header and payload as separate USB bulk transfers (also supported in streaming sends)
- **Custom VID/PID device discovery:** `MtpDevice::list_devices_with_known()` and `MtpDeviceBuilder::known_devices()` to include devices with non-standard USB descriptors in enumeration and open
- **`MtpDeviceBuilder::open_nusb_device()`** escape hatch for consumers doing their own USB enumeration or hotplug watching
- **Permissive interface scan on open:** two-pass scan (strict MTP class first, then endpoint-layout fallback) for devices with non-standard interface descriptors
- **macOS `SetConfiguration(1)` retry:** automatically recovers when IOKit doesn't publish interface services for vendor-class devices

### Fixed

- Gate macOS-only `is_interface_unpublished` helper with `#[cfg(target_os = "macos")]` to fix dead-code warning on non-macOS builds

Thanks to [@kelchm](https://github.com/kelchm) for contributing the low-level primitives ([#4](https://github.com/vdavid/mtp-rs/pull/4)).

## [0.9.1] - 2026-04-08

### Fixed

- Virtual device's `handle_move_object` now emits MTP events (`ObjectInfoChanged` + `StorageInfoChanged`), fixing a bug where consumers' event loops had no signal to refresh directory listings after a move

## [0.9.0] - 2026-04-08

### Added

- `pause_watcher(serial)` API returning an RAII `WatcherGuard` that suppresses filesystem events while alive, preventing a race condition where stale OS deletion events corrupt the object tree after a rescan
- `WatcherGuard` re-exported from crate root

## [0.8.0] - 2026-04-07

### Added

- `rescan_virtual_device(serial)` API to force-sync the virtual device's in-memory object tree with the filesystem, removing stale entries and adding new ones with proper MTP event queuing
- Active-state registry for live `VirtualTransport` instances, with `Drop`-based cleanup
- `RescanSummary` type re-exported from crate root

## [0.7.2] - 2026-04-03

### Fixed

- Fix fs watcher dedup on macOS: skip FSEvents startup event for the backing directory itself (empty relative path) that produced a spurious `ObjectAdded`
- Bump `actions/checkout` from v4 to v5 in CI (Node.js 20 deprecation)

## [0.7.0] - 2026-04-03

### Added

- `MtpDevice` now implements `Clone` (cheap — wraps `Arc` internally), enabling consumers to clone the device for concurrent event polling

### Fixed

- Fix fs watcher dedup on macOS: event processing moved from watcher callback (FSEvents thread) to `receive_interrupt` (caller thread), eliminating cross-thread timing issues
- Fix incorrect `progress.percent().unwrap_or(0.0)` in `FileDownload::collect_with_progress` doc example (`percent()` returns `f64`, not `Option`)

### Changed

- 13 doc examples converted from `ignore` to `no_run` with hidden boilerplate (now compile-checked, catches API drift)

## [0.6.1] - 2026-04-03

### Fixed

- Fix flaky `fs_watcher_dedup` test on macOS: assert on `ObjectAdded` count instead of total event count, since extra `StorageInfoChanged` events may be generated

## [0.6.0] - 2026-04-02

### Added

- Filesystem watcher for virtual devices: when `watch_backing_dirs` is `true`, the virtual device detects files created or removed directly in backing directories (bypassing MTP) and emits `ObjectAdded`/`ObjectRemoved` events, matching real device behavior
- `VirtualDeviceConfig::watch_backing_dirs` field to opt in/out of filesystem watching
- `notify` v8 dependency (optional, gated behind `virtual-device` feature)

### Changed

- **Breaking:** MSRV raised from 1.79 to 1.85
- Upgraded `notify` from v7 to v8 (drops unmaintained `instant` transitive dep)
- Upgraded `thiserror` from v1 to v2 (faster proc-macro compilation, no API changes)
- Unpinned `proptest` dev-dependency (was pinned to `=1.5.0` for MSRV 1.79)

## [0.5.1] - 2026-04-01

### Fixed

- Fix clippy `needless_borrow` warnings on Rust 1.79 (MSRV) in virtual device module

## [0.5.0] - 2026-04-01

### Added

- `virtual-device` feature for testing MTP client code without USB hardware
  - `VirtualTransport` implements the `Transport` trait against local filesystem directories, speaking the full MTP/PTP binary protocol so `MtpDevice`, `Storage`, and `PtpSession` work unchanged
  - `MtpDevice::builder().open_virtual(config)` creates a virtual device directly
  - `register_virtual_device()` / `unregister_virtual_device()` integrate with `list_devices()`, `open_by_location()`, and `open_by_serial()`
  - Supports 16 MTP operations: list/get/delete/move/copy/rename objects, upload files, create folders, storage info, device info, events
  - Path traversal protection on all write operations
  - Configurable `event_poll_interval` to avoid CPU spin in event loops
  - Read-only storage support
  - Zero changes to existing code paths when the feature is disabled

## [0.4.2] - 2026-04-01

### Fixed

- Send `OpenSession` with `transaction_id=0` (session-less) per PTP spec — fixes Kindle and other strict PTP devices rejecting the session ([#2](https://github.com/vdavid/mtp-rs/pull/2), thanks [@num13ru](https://github.com/num13ru))
- Fix stale `next_event()` docs after timeout removal
- Fix README indentation broken by PR #2

## [0.4.1] - 2026-03-24

### Fixed

- Detect vendor-specific MTP devices (e.g. Amazon Kindle) that use USB class 0xFF with non-standard subclass/protocol ([#1](https://github.com/vdavid/mtp-rs/issues/1))

## [0.4.0] - 2026-03-20

### Changed

- Replaced platform-specific IOKit/location_id code with nusb's cross-platform `port_chain()` + `bus_id()`
- **Breaking:** `location_id` values will differ from previous versions (now derived from USB topology instead of macOS IOKit)
- Fixed timeout race condition: `receive_bulk` now leaves USB transfers pending on timeout instead of cancelling them, preventing data loss on retry
- `receive_interrupt()` now awaits indefinitely for events (no timeout); callers should use async cancellation
- Switched from `std::sync::Mutex` to `futures::lock::Mutex` for async-safe locking across `.await` points
- Re-added `futures-timer` dependency for async timeout support

### Removed

- Removed `io-kit-sys` and `core-foundation` macOS dependencies (location info now provided by nusb)
- **Breaking:** Removed `event_timeout`, `DEFAULT_EVENT_TIMEOUT`, `set_event_timeout()`, `event_timeout()`, and `open_with_timeouts()` from `NusbTransport`
- **Breaking:** Removed `event_timeout()` from `MtpDeviceBuilder`

## [0.3.0] - 2026-03-20

### Removed

- Removed `futures-timer` dependency (timeouts now handled by nusb internally)

### Changed

- **Breaking:** Upgraded `nusb` dependency from 0.1 to 0.2
- **Breaking:** MSRV raised from 1.75 to 1.79
- **Breaking:** `UsbDeviceInfo::open()` now returns `Result<nusb::Device, nusb::Error>` instead of `Result<nusb::Device, std::io::Error>`
- **Breaking:** Removed `NusbTransport::bulk_in_endpoint()`, `bulk_out_endpoint()`, `interrupt_in_endpoint()` accessors
- Improved MTP device detection: can now detect composite MTP devices without opening them (nusb 0.2 exposes interface info on `DeviceInfo`)
- Transport internals now use nusb 0.2's `Endpoint` pattern with `transfer_blocking` instead of single-shot methods

## [0.2.0] - 2026-03-17

### Added

- `Storage::list_objects_stream()` — streaming object listing that yields `ObjectInfo` items one at a time from USB, with `total()` and `fetched()` for progress reporting
- `ObjectListing` struct for iterating over streamed results
- Reproducible benchmark suite (`mtp-bench` crate at `benchmarks/mtp-rs-vs-libmtp/`) comparing mtp-rs against libmtp
- Benchmark results in README: mtp-rs is 1.06x–4.04x faster across all operations
- Release process documentation (`docs/releasing.md`)

### Changed

- `list_objects()` refactored to use `list_objects_stream()` internally — no behavior change

## [0.1.0] - 2026-02-20

Initial release targeting modern Android devices.

### Added

- Connect to Android phones/tablets over USB
- List, download, upload, delete, move, and copy files
- Create and delete folders
- Stream large file downloads with progress tracking
- Listen for device events (file added, storage removed, etc.)
- Two-layer API: high-level `mtp::` and low-level `ptp::`
- Runtime-agnostic async design (works with tokio, async-std, etc.)
- Pure Rust implementation using `nusb` for USB access
- Smart recursive listing that auto-detects Android and uses manual traversal
- `Storage::list_objects_recursive_manual()` for explicit manual traversal
- `Storage::list_objects_recursive_native()` for explicit native MTP recursive listing
- Android device detection via `"android.com"` vendor extension
- Integration tests organized into `readonly` and `destructive` categories
- Serial test execution to avoid USB device conflicts
- Diagnostic example (`examples/diagnose.rs`)

### Fixed

- MTP device detection for composite USB devices (class 0)
  - Most Android phones are composite devices with MTP as one interface
  - Now properly inspects interface descriptors to find MTP
- Large MTP data containers (>64KB) now handled correctly
  - Data spanning multiple USB transfers is reassembled before parsing
- Recursive listing now works on Android devices
  - Android ignores `ObjectHandle::ALL`; we detect this and use manual traversal
- Integration tests now use `Download/` folder instead of root
  - Android doesn't allow creating files/folders in storage root

### Changed

- `list_objects_recursive()` now automatically chooses the best strategy:
  - Android devices: manual folder-by-folder traversal
  - Other devices: native recursive, with fallback to manual if results look incomplete

### Not included (by design)

- MTPZ (DRM extension for old devices)
- Playlist and metadata syncing
- Vendor-specific extensions
- Legacy device quirks database
