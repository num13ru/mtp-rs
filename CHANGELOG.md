# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
