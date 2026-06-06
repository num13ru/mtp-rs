# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file covers both published crates in the workspace:

- `mtp-rs` (the library)
- `mtp-rs-cli` (the CLI binary, new in this release)

Entries are grouped by release. Each entry tags which crate it applies to with **[lib]**, **[cli]**, or **[workspace]** for repo-wide changes.

## [Unreleased]

### Added

- **[lib] `MtpDevice::supports_upload()` and `DeviceInfo::supports_upload()`.** Returns true when the device advertises both `SendObjectInfo` (0x100C) and `SendObject` (0x100D), the two-phase object creation flow. Read-only devices (PTP cameras like the Panasonic Lumix DMC-TZ61 from [#12](https://github.com/vdavid/mtp-rs/issues/12)) typically don't advertise these, so consumers can skip write attempts up front. Mirrors the existing `supports_rename()`. Note the result means "worth attempting", not "guaranteed": Fuji cameras advertise write support yet reject writes per-operation.
- **[lib] 13 new `OperationCode` variants** covering the rest of the standard PTP set (`FormatStore`, `ResetDevice`, `SelfTest`, `SetObjectProtection`, `PowerDown`, `TerminateOpenCapture`, `InitiateOpenCapture`) and the common MTP object-property extensions (`GetObjectPropsSupported`, `GetObjectPropDesc`, `GetObjectPropList`, `SetObjectPropList`, `GetObjectReferences`, `SetObjectReferences`). These previously decoded as `Unknown(...)`, which made diagnostic output (the `ptp_diagnose` example, the CLI's `doctor`) harder to read. Spotted on the Lumix DMC-TZ61's operations list in [#12](https://github.com/vdavid/mtp-rs/issues/12). Technically breaking for exhaustive matches on `OperationCode`, but matches realistically end in an `Unknown(_)` or `_` arm.
- **[lib] Device reset: `Transport::reset_device()` and `PtpDevice::reset_device()`.** Sends the USB Still Image Class Device Reset request (`bRequest=0x66`), clears halted bulk endpoints, and drains stale bulk data. Recovers a device whose PTP state machine is stuck after an interrupted transfer (symptoms: every command fails with "Transaction ID mismatch" or "expected Response container type" errors). Works without a PTP session, which is the point: a stuck device can't answer `OpenSession`. From [#12](https://github.com/vdavid/mtp-rs/issues/12), where a Ctrl-C'd listing left the Lumix DMC-TZ61 needing a battery pull.
- **[cli] New `mtp-rs reset` command** exposing the device reset, with `--device`/`--location`/`--json` support and a post-reset `GetDeviceInfo` verification. Documented in `docs/cli.md` with a troubleshooting entry.

### Fixed

- **[lib] Unparseable datetimes in `ObjectInfo` no longer fail the whole listing.** The Panasonic Lumix DMC-TZ61 ([#12](https://github.com/vdavid/mtp-rs/issues/12)) reports `20480000T000000` (month 0, day 0) as its "no date" sentinel in `DateCreated`/`DateModified`. `unpack_datetime` treated that as a hard error, which failed the `ObjectInfo` parse, which failed the entire (recursive) listing: hundreds of photos invisible because of one metadata field. Receive-side parsing is now lenient: an unparseable datetime becomes `None` (the length-prefixed string is still consumed correctly, so the fields after it stay aligned). Send-side packing stays strict.
- **[lib] Devices stay usable after a mid-transfer cancel.** `cancel_transfer` now polls SIC GET_DEVICE_STATUS (0x67) after the bulk and interrupt drains, until the device stops reporting Device_Busy, clearing any bulk endpoint halts the status response reports. SIC-compliant devices (PTP cameras) wait for this poll before accepting new operations; without it the Lumix DMC-TZ61 ([#12](https://github.com/vdavid/mtp-rs/issues/12)) timed out on every operation after a "successful" cancel until a battery pull. The poll runs strictly after the drains (polling between cancel and drain breaks Android, which doesn't implement the request at all and harmlessly fails it).
- **[lib] Endpoint halts are cleared after STALL.** Cameras stall a bulk endpoint to signal unsupported operations, and the halt persists across process restarts. Previously a `ptp_diagnose` property probe wedged the endpoint so the next run failed at `GetDeviceInfo` with "endpoint stalled". Every bulk/interrupt completion site now clears the halt before surfacing the error.
- **[lib] PTP strings truncate at the first NUL.** `unpack_string` stripped exactly one trailing NUL, but the Lumix pads its serial number to a fixed width with multiple NULs, so one leaked into the decoded `String` (showed up as an editor encoding warning on logs). Per spec, anything from the first NUL on is padding.

### Changed

- **[workspace] Destructive integration tests now skip cleanly on read-only devices.** They check `supports_upload()` before writing (same pattern as the existing rename-support skip), and the test harness logs the specific skip reason (no device, no upload support, no writable folder) instead of a generic setup-failed message. The recursive file-search helper also logs listing errors instead of silently treating them as "no file found". Triggered by the Panasonic Lumix DMC-TZ61 report in [#12](https://github.com/vdavid/mtp-rs/issues/12).
- **[workspace] Download tests find their test file much faster.** The recursive fallback is a breadth-first streaming search that stops at the first size match instead of listing the whole storage first (10+ minutes on PTP cameras, seconds now), the find is cached across tests in a run, and a new `MTP_TEST_READFILE` env var pins an exact path with no searching at all.
- **[workspace] The `diagnose` example bounds its recursive listing to 200 objects.** The unbounded version ran 10+ minutes on cameras with slow per-object metadata fetches, tempting users into mid-traversal Ctrl+C, which wedges some devices.

## [0.19.0] - 2026-05-30

Inadvertent no-op re-release: the source is byte-identical to 0.18.0 (only `Cargo.toml` / `Cargo.lock` / this changelog differ). It was published while reconciling a downstream (Cmdr) build failure that looked like 0.18.0 lacked the `UploadError` API; the real cause was a transient stale lockfile during a parallel build, not a missing release. 0.18.0 (lib) / 0.2.0 (cli) already shipped `UploadError` correctly — see below. 0.19.0 (lib) / 0.3.0 (cli) add nothing; they exist only because they were published. Prefer 0.18.0 / 0.2.0.

## [0.18.0] - 2026-05-30

### Changed

- **[lib] Breaking: `Storage::upload` and `upload_with_progress` now return `Result<ObjectHandle, UploadError>`** (was `Result<ObjectHandle, Error>`). PTP uploads are two-phase: `SendObjectInfo` creates the object on the device (yielding a handle), then `SendObject` streams the bytes. If the data phase fails or is cancelled, the device is left holding a partial (often empty or truncated) object that the caller previously had no way to address. The new `UploadError` carries `source: Error` plus `partial: Option<ObjectHandle>` — `Some(handle)` when `SendObjectInfo` succeeded but the data phase didn't, `None` when no object was created. The library does **not** auto-delete the partial: that would issue hidden USB I/O to a possibly-disconnected device, the leave-vs-delete behavior is device-dependent, and PTP's model is designed so a failed `SendObject` can be retried against the same handle (resume). The consumer owns the cleanup-or-resume decision. `From<UploadError> for Error` keeps `?` ergonomic in an `Error` context (the `partial` is dropped unless the caller matches on `UploadError`). `UploadError` is re-exported at the crate root.
- **[lib] Virtual device now creates the object at `SendObjectInfo` time**, matching real devices (an empty placeholder file for files, the directory for folders), so a mid-stream or cancelled upload leaves a real, queryable, deletable object at the surfaced `partial` handle. `SendObject` then overwrites the placeholder and emits `ObjectInfoChanged` (rather than a second `ObjectAdded`), preserving the one-`ObjectAdded`-per-upload watcher-dedup contract.
- **[cli] `put` now cleans up the partial object on upload failure.** A failed/cancelled `mtp-rs put` deletes the partial object the device created (best-effort; `put` has no resume story), then reports the underlying error. Bumped to 0.2.0 because the lib dependency moved to a breaking version and the failure-path behavior changed.

## [0.17.0] - 2026-05-27

### Added

- **[cli] New `mtp-rs-cli` crate (initial release at 0.1.0).** Ships a universal MTP file transfer CLI under the binary name `mtp-rs`. Subcommands: `devices`, `info`, `ls`, `put`, `get`, `mkdir`, `rm`, `rename`, `mv`, `cp`, `doctor`. Every command supports `--json` for automation. Stable exit codes (0/2/3/4/5/6/7). Streaming uploads and downloads with progress, `--verify` for upload byte-comparison, POSIX-like absolute remote paths. See `crates/mtp-rs-cli/docs/cli.md` for the full reference. Originally contributed by [@dtretyakov](https://github.com/dtretyakov) in [#11](https://github.com/vdavid/mtp-rs/pull/11).
- **[lib] Match-reason on enumerated devices.** `MtpDeviceInfo::match_reason` and `UsbDeviceInfo::match_reason` carry a new `MtpMatchReason` enum (`StandardClass`, `InterfaceString`, `KnownVidPid`, `OpenedDescriptorScan`) explaining why a USB device was classified as MTP. Useful for the CLI's `doctor` command and for any consumer that wants to surface match provenance. Both info structs are `#[non_exhaustive]`, so this is additive.
- **[lib] Garmin-style `MTP` interface-string detection.** Devices that expose MTP on a vendor-class (`0xff/0xff`) interface but advertise an `interface_string` of `MTP` are now classified correctly. Verified on Garmin Venu 2/2S; likely covers other Garmin Connect IQ devices too.

### Changed

- **[workspace] Repo is now a Cargo workspace.** The library is at `crates/mtp-rs/`, the CLI is at `crates/mtp-rs-cli/`. Public lib API is unchanged. Library users can keep `mtp-rs = "0.17"` in their `Cargo.toml` and rebuild against the new version without code changes. The workspace split keeps the library free of CLI-only dependencies (`clap`, `serde`, `serde_json`, `tokio`) even as optional features. The `benchmarks/mtp-rs-vs-libmtp/` crate moved to a path dep on `../../crates/mtp-rs`.
- **[lib] Virtual-device watcher restored to `RecommendedWatcher`.** The CLI PR temporarily swapped to `PollWatcher` to work around a local test flake. That made every virtual-device user scan their backing dirs 20×/sec forever. Restored to the kernel-driven native watcher; the existing `poll_event_with_retry` test pattern already handles FSEvents latency on macOS.

### Notes

- The `mtp-rs` binary used to live in the library crate behind a `cli` feature. It has moved to the new `mtp-rs-cli` crate. The `cli` feature on the lib crate is gone. Installation is now `cargo install mtp-rs-cli`. The binary name itself is still `mtp-rs`.
- Library MSRV stays at 1.85. The CLI crate also targets MSRV 1.85.

## [0.16.0] - 2026-05-23

### Added

- **Event-driven backing-dir drain for virtual devices.** Tests that recreate a virtual device's backing dir externally previously had to sleep ≥600 ms (macOS FSEvents worst-case) after the writes before resuming the watcher, or risk stale events corrupting the object tree. The new observation surface replaces that with actual quiescence:
  - `dropped_paths_since_pause(serial) -> Vec<PathBuf>` returns the canonical paths the watcher has dropped while paused, oldest first. This is the **primary primitive** — compose your own pattern on top (sentinel-file drain, event-count quiescence, per-subdir filter).
  - `was_path_dropped(serial, suffix) -> bool` is a thin convenience wrapper for the sentinel-file pattern: write a uniquely-named file as the LAST fixture step, poll this until it returns `true`. Per-directory FS-event ordering on every supported `notify` backend means every earlier write to the same directory already arrived. Suffix-match sidesteps `/tmp` ↔ `/private/tmp` canonicalization.
  - `clear_dropped_paths(serial)` empties the ring after a successful drain.
  - The ring is capped at `DROPPED_PATHS_CAP = 1024` entries (publicly visible constant; ~160 KB worst case). Oldest evicted on push past the cap.
- **Refcounted pause/resume.** `pause_watcher` now increments an internal `pause_count` instead of flipping a `bool`; `WatcherGuard::drop` decrements it. The watcher actually resumes only when the last guard drops, so multiple concurrent test drains compose correctly. Previously, two concurrent drains would race: one would resume while the other's events were still in flight. Backwards-compatible API — the change is internal to `VirtualDeviceState` (`pub(super)` field), and single-guard usage behaves identically.

### Changed

- `WatcherGuard::drop` no longer unconditionally clears the paused flag; it decrements the refcount and only sets the watcher to resume when the count reaches zero. Single-guard usage (the common case) is unchanged. Multi-guard usage now composes correctly instead of racing.

### Notes

- Behind the existing `virtual-device` feature flag; production consumers without that feature compile zero of this. Memory cost (the dropped-paths ring) lives entirely in the virtual-device code path.
- The watcher integration is exercised end-to-end by downstream consumers' E2E suites (Cmdr's MTP Playwright lane uses the sentinel-file pattern); the library's own unit tests cover the observation API, refcount composition, and ring eviction.

## [0.15.0] - 2026-05-19

### Added

- **Cooperative cancellation for long list and delete operations via `CancelToken`.** New `cancel` module exposes `CancelToken` (`Arc<AtomicBool>`-backed, `Clone + Send + Sync`) and re-exports it at the crate root. `Storage::list_objects_with_cancel`, `Storage::list_objects_stream_with_cancel`, and `Storage::delete_with_cancel` accept an `Option<&CancelToken>`. When the token flips, the per-handle iteration inside `ObjectListing::next` returns `Err(Error::Cancelled)` at the next per-object boundary (typically within one `GetObjectInfo` USB roundtrip), instead of running the full 1k+ entry loop to completion. The token is one-way and cheap to clone, so consumers make a fresh one per logical op.
- **`CancelToken::from_arc(Arc<AtomicBool>)`** wraps a consumer-owned atomic so existing cancellation state (a write-op intent flag, a shared abort signal) flips the token directly. No second polling task, no two-way sync.
- The existing `Storage::list_objects` / `list_objects_stream` / `delete` entry points stay for backwards compatibility. They now delegate to the `_with_cancel` variants with `None`.

### Notes

- Streaming downloads keep their existing USB SIC class-cancel path via `FileDownload::cancel`. That handles a different problem (one long bulk-IN to drain) and stays unchanged.
- Per-handle cancellation only fires at per-object boundaries, which is where slow listings actually spend their time. Mid-USB-transaction cancel for these ops would be both more complex and less safe (drain semantics on a half-finished `GetObjectInfo` are device-dependent).

## [0.14.0] - 2026-05-15

### Added

- **Negotiated USB link speed on enumerated devices.** `MtpDeviceInfo::speed` and `UsbDeviceInfo::speed` now carry `Option<UsbSpeed>` populated from `nusb::DeviceInfo::speed()`. `UsbSpeed` is a five-variant enum (`Low`, `Full`, `High`, `Super`, `SuperPlus`) re-exported at the crate root, so consumers can surface negotiated speed (USB 1.0 low / 1.1 full / 2.0 / 3.2 Gen 1 / 3.2 Gen 2) without adding a direct `nusb` dependency. The value is the slowest of host port, cable, and device, which is useful for diagnosing "fast device on a USB-2 charging cable" cases.

### Changed

- **Breaking**: `MtpDeviceInfo` and `UsbDeviceInfo` gained a `speed: Option<UsbSpeed>` field. Both structs are now marked `#[non_exhaustive]`, so future field additions are non-breaking. Consumers that constructed either struct via struct literal (rare — these are documented as return types from `list_devices()`) now need `..` or named construction.

## [0.13.3] - 2026-05-05

### Fixed

- **`PtpDevice::get_device_info()` now handles devices that send the container header and payload in separate USB bulk transfers.** Some spec-compliant MTP devices (Garmin Forerunner 955, observed) send the 12-byte data container header in one bulk transfer and the payload in a follow-up transfer. The session-less `GetDeviceInfo` path parsed straight from the first transfer and bailed with `data container length mismatch: header says N, have 12`. The in-session `PtpSession::execute_with_receive` already handled this; the fix mirrors the same multi-transfer accumulation in the session-less path. Reported by [@dasJ](https://github.com/dasJ) on [#10](https://github.com/vdavid/mtp-rs/pull/10).

### Changed

- `PtpDevice::transport` is now `Arc<dyn Transport>` instead of `Arc<NusbTransport>`. Internal change; no public API impact. Enables mock-based unit testing of session-less paths.
- AGENTS.md now codifies the multi-transfer receive convention so future code paths that parse a `DataContainer` know to accumulate USB transfers until the full container is in hand.

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
