# Windows WPD backend + backend-neutral `mtp::` plan

## Status

- **Phase 0 (spike): DONE, hardware-verified.** A throwaway pure-Rust WPD spike enumerated, opened,
  listed (multi-level), and downloaded+byte/SHA-verified a file from a real Pixel 9 Pro XL on
  Windows. See "Phase 0 findings" below for the proven facts that the rest of this plan relies on.
- **Phase 1 (backend seam + neutral `mtp::`): DONE.** Landed on branch `feat/windows-wpd-backend`.
  `MtpBackend` trait + `UsbBackend` (sole impl), backend-neutral `mtp::` types/errors, opaque `u64`
  handles, three download patterns, neutral `mtp::Error` (incl. typed `StaleHandle`/`ExclusiveAccess`/
  `PermissionDenied`), `NewObjectInfo` neutralized, `session()` dropped, `capabilities()` added,
  upload bounds relaxed to accept borrowed streams/callbacks. `just` fully green (412 tests incl. the
  new conformance suite); existing USB/virtual/mock behavior unchanged (quirks, partial-handle, SIC
  cancel, recovery all preserved). Exit gate met: `mtp-rs-cli` green, and Cmdr migrated + verified
  green (3029 default tests + 19 MTP virtual-device E2E, incl. upload-partial/cancel/stale-handle).
  Cmdr's dep is temporarily pinned to this worktree via `path`; swap to a crates.io version when the
  neutral API ships.
- Phases 2-5: not started. Phase 2 (WpdBackend) is next, on Windows.

Breaking changes to the public API are **in scope and expected** — the library is early-stage and the
goal is the elegant end state, not source compatibility. The one hard constraint is **no behavioral
regression on existing backends** (USB/nusb, virtual device, mock): they must run the same code paths
and pass the same tests after the refactor.

## Why this exists

`mtp-rs` reaches devices via `nusb` (raw USB). On Windows that fails for phones: `nusb` can only
claim interfaces bound to the WinUSB driver, but Windows binds phones to its own WPD/MTP driver, so
`open()` returns `"incompatible driver is installed for this interface"` (GitHub issue #13). The
sanctioned way to talk MTP on Windows is the **WPD (Windows Portable Devices) COM API**.

The key architectural fact: **WPD is not a USB transport.** It speaks MTP *for* you and exposes a
high-level object model; it owns the session/transaction state, and its raw-MTP passthrough
(`IPortableDeviceService` + `WPD_COMMAND_MTP_EXT_*`) blocks the standard operations
(`GetObject`/`GetStorageIDs`/`SendObject`/…) that `mtp-rs` would need. So WPD **cannot** sit under the
existing `transport::Transport` trait and reuse `ptp::`. It is a parallel high-level stack.

Therefore the abstraction seam is the **`mtp::` layer**, not `transport::`. PTP-over-USB and
WPD-over-COM become two implementations of one backend-neutral high-level API.

## North star

> **`mtp::` becomes a backend-neutral portable-device API. `ptp::` stays the USB-PTP low-level API
> (cameras, protocol work). PTP-over-USB and WPD-over-COM are two implementations of `mtp::`.**

Today `mtp::` silently leaks PTP/USB internals: handles *are* PTP wire values, `MtpDevice::session()`
returns a `PtpSession`, `mtp::DeviceInfo` is the PTP wire struct, and capability checks lean on
`OperationCode`. That was fine with one backend; with WPD it is a lie. The elegant move is to make
`mtp::` honest and neutral, and keep `ptp::` as the rich, USB-specific escape hatch.

## Goals and non-goals

### Goals

- Real feature parity on Windows for the high-level `mtp::` API: enumerate, open, list, download
  (incl. ranged/resumable), upload (incl. progress), delete, create folder, rename, move, copy,
  object info, thumbnails. Events may land later (see open decisions).
- A genuinely backend-neutral `mtp::` surface: opaque handles, neutral types, neutral errors.
- One **backend-conformance test suite** that runs against every backend, giving provable parity.
- Zero behavioral regression on USB/virtual/mock backends.
- Pure Rust, no C deps (WPD via the official `windows` crate; see "Dependency" — this is consistent
  with the project's principles, `nusb` already FFIs into Windows DLLs via `windows-sys`).

### Non-goals

- New end-user features beyond current capabilities (this is a re-architecture, not a feature push).
- Making `ptp::` work over WPD (impossible — WPD blocks the raw opcodes).
- Forcing path-based addressing as the primary currency (handles stay primary; path helpers remain a
  convenience layer).
- A generic plugin/registry for arbitrary backends (two `cfg`-selected backends; YAGNI).
- macOS ImageCaptureCore backend (the architecture must *allow* it later; we do not build it now).
- MTPZ, vendor extensions, playlist/metadata sync (unchanged from existing scope).

## Architecture

### Layering after the change

```
mtp::  (MtpDevice, Storage, FileDownload, ObjectListing)   <-- backend-neutral high-level API
  |  Box<dyn MtpBackend>
  +-- UsbBackend   (PTP over Transport: nusb | virtual | mock)   <-- wraps today's ptp:: + transport::
  +-- WpdBackend   (WPD over COM, cfg(windows))                  <-- new, COM worker-thread actor

ptp::  (PtpDevice, PtpSession)   <-- USB-PTP low-level API, unchanged, USB-only by nature
transport::  (Transport trait, nusb, virtual_device, mock)   <-- unchanged
```

The **virtual device stays a `Transport` under `UsbBackend`** — it is not a third backend. Every
existing virtual-device test keeps exercising the real `UsbBackend` code path unchanged. WPD is the
only genuinely new backend.

### Module organization

```
src/
├── mtp/
│   ├── device.rs        # MtpDevice façade over Box<dyn MtpBackend>, MtpDeviceBuilder, selection
│   ├── storage.rs       # Storage façade (delegates to backend); conveniences live here
│   ├── backend/
│   │   ├── mod.rs       # MtpBackend trait, Backend selector enum, shared neutral helpers
│   │   ├── usb.rs       # UsbBackend: implements MtpBackend over PtpSession (today's logic)
│   │   └── wpd/         # cfg(windows) — WpdBackend
│   │       ├── mod.rs   # WpdBackend, open/selection, capability probe
│   │       ├── actor.rs # COM worker thread, message protocol, lifecycle
│   │       ├── com.rs   # thin safe wrappers over windows-rs WPD calls
│   │       ├── ids.rs   # ObjectHandle/StorageId <-> WPD string-ID bimap
│   │       ├── props.rs # WPD property-bag get/set, PROPVARIANT/PWSTR helpers
│   │       └── consts.rs# the one hand-defined constant (WPD_DEVICE_OBJECT_ID = "DEVICE")
│   ├── object.rs        # ObjectInfo, NewObjectInfo, ObjectFormat (neutral)
│   ├── device_info.rs   # neutral mtp::DeviceInfo, mtp::Capabilities
│   ├── error.rs         # neutral mtp::Error (or extend top-level error.rs)
│   ├── event.rs         # DeviceEvent (neutral)
│   └── stream.rs        # FileDownload, ObjectListing over Pin<Box<dyn Stream + Send>>
├── ptp/                 # unchanged (public low-level API)
├── transport/           # unchanged
└── error.rs             # crate error root
```

### The `MtpBackend` trait (indicative, not final)

A trait (not enum dispatch) because the per-call allocation is noise against USB/COM latency, each
backend stays self-contained in its module, and a future backend is a new file rather than edits to
every method. `MtpDevice`/`Storage` are thin concrete façades over `Box<dyn MtpBackend>`, so users
never see generics or trait objects.

```rust
#[async_trait]
pub(crate) trait MtpBackend: Send + Sync {
    fn device_info(&self) -> &DeviceInfo;          // neutral
    fn capabilities(&self) -> &Capabilities;       // neutral

    async fn storages(&self) -> Result<Vec<StorageInfo>>;
    async fn list(&self, parent: ObjectHandle, cancel: Option<&CancelToken>)
        -> Result<ObjectListing>;                  // streaming, cancellable
    async fn object_info(&self, obj: ObjectHandle) -> Result<ObjectInfo>;

    // Two download primitives (see "Download consolidation"): streaming + single-shot range.
    // download_to_vec and WindowedDownload are façade conveniences over read_range.
    async fn download(&self, obj: ObjectHandle, range: ByteRange) -> Result<FileDownload>;
    async fn read_range(&self, obj: ObjectHandle, offset: u64, len: Option<u64>) -> Result<Vec<u8>>;
    async fn thumbnail(&self, obj: ObjectHandle) -> Result<Vec<u8>>;

    async fn upload(&self, parent: ObjectHandle, info: NewObjectInfo,
                    data: BulkStream<'_>, progress: Option<ProgressFn>)
        -> Result<ObjectHandle, UploadError>;
    async fn create_folder(&self, parent: ObjectHandle, name: &str) -> Result<ObjectHandle>;
    async fn delete(&self, obj: ObjectHandle, cancel: Option<&CancelToken>) -> Result<()>;
    async fn move_object(&self, obj: ObjectHandle, new_parent: ObjectHandle) -> Result<()>;
    async fn copy_object(&self, obj: ObjectHandle, new_parent: ObjectHandle) -> Result<ObjectHandle>;
    async fn rename(&self, obj: ObjectHandle, new_name: &str) -> Result<()>;

    async fn next_event(&self) -> Result<DeviceEvent>;   // capability-gated; may be deferred
    async fn close(self: Box<Self>) -> Result<()>;
}
```

Conveniences (`download_to_vec`, recursive listing, etc.) are default-implemented once on the façade
in terms of these primitives, so both backends get them for free.

### Eight design moves

1. **`MtpBackend` trait + `Box<dyn MtpBackend>` façade.** UsbBackend wraps today's `PtpSession`
   logic; WpdBackend is new. Virtual/mock ride UsbBackend via their `Transport`s.
2. **Opaque handles.** `ObjectHandle`/`StorageId` become opaque, `Copy`, session-scoped tokens
   (`u64`), documented as valid only within one open device session — **not** wire values. UsbBackend
   uses the raw value; WpdBackend keeps a session-local token↔string-ID bimap (`ids.rs`). This
   generalizes a truth the repo already documents (Android handles are unstable).
3. **Neutral `mtp::` types.** New `mtp::DeviceInfo` (serial, manufacturer, model) and
   `mtp::Capabilities` (`can_upload`, `can_delete`, `can_rename`, `can_move`, `can_copy`,
   `can_create_folder`, `supports_partial_download`, `supports_thumbnails`, `supports_events`).
   WpdBackend derives caps from WPD command support; UsbBackend from the PTP operations list. The
   ad-hoc `supports_rename()`/`supports_upload()`/`is_android()` accessors fold into this (`is_android`
   becomes an internal quirk flag, surfaced via a neutral `Capabilities`/quirk field if consumers need
   it). `OperationCode` stops leaking through `mtp::`.
4. **Drop `MtpDevice::session()`.** No `PtpSession` exists behind WPD. Raw PTP access stays available
   through `ptp::` directly (honestly USB-only).
5. **Neutral error model.** `mtp::Error` exposes backend-agnostic variants with backend detail
   preserved in a `source` for diagnostics. Both PTP `ResponseCode`s and WPD `HRESULT`s map into the
   neutral set (table below). `ptp::` keeps its rich code-level errors. Predicates
   (`is_retryable()`, `is_exclusive_access()`, stale-handle detection) become backend-neutral.
6. **Automatic backend selection + override.** `open_first()` picks per platform/device: Windows →
   WPD; Linux/macOS → USB; virtual → explicit. `MtpDeviceBuilder::backend(Backend::Auto | Usb | Wpd)`
   is the escape hatch (e.g. force raw USB to a Zadig-bound camera on Windows). Default `Auto`.
7. **Unified cancellation contract.** `FileDownload::cancel().await` is clean+immediate on both
   (WPD releases the `IStream`; USB does the SIC class-cancel drain). Dropping without `cancel()` is
   *safe* on both (WPD: COM `Release`; USB: lazy next-op recovery, which already exists). The USB
   path's `debug_assert`-on-drop softens to match. Documented once at the `mtp::` layer.
8. **Honest platform docs.** `ptp::` is the USB-PTP API: on Windows it only reaches WinUSB-bound
   devices (cameras, or Zadig); phones bound to the WPD driver are reached through `mtp::`/WPD.

### Download consolidation (corrected after consumer mapping)

There are not 5 but ~9 download entry points today, and they fall into **three genuinely distinct
patterns** that the real consumers depend on — so the consolidation merges *within* each pattern, it
does **not** flatten them into one:

```rust
enum ByteRange { Full, From(u64), Range { offset: u64, len: u64 } }
```

1. **Streaming, holds the session** — `download(&self, obj, ByteRange) -> FileDownload`. Subsumes
   `download_stream` + `download_stream_from_offset`. The **CLI** uses this (`get`, `put --verify`).
   UsbBackend: `GetObject` / `GetPartialObject64`; WpdBackend: `IStream` (+ `Seek` for offset).
2. **Windowed, releases the session between windows** — `download_windowed(&self, obj, ByteRange,
   window_size) -> WindowedDownload`, pulled via `next_window()`. Subsumes
   `download_windowed` / `download_windowed_from_offset` / `download_windowed_default`. **Cmdr's
   primary download path** (`file_ops.rs`). This is the real suspend/resume mechanism: each window is
   one bounded read and the device is listable between windows. Must be preserved. UsbBackend:
   `GetPartialObject64` per window; WpdBackend: bounded `IStream` reads (no exclusive session to
   release, but the same chunked-pull shape holds).
3. **Buffered convenience** — `download_to_vec(&self, obj, ByteRange) -> Vec<u8>`. Subsumes
   `download` + `download_partial` + `download_partial_64`. Plus `thumbnail(&self, obj) -> Vec<u8>`.

The backend trait exposes two primitives — a streaming `download(obj, range)` and a single-shot
`read_range(obj, offset, len) -> Vec<u8>`; `download_to_vec` and `WindowedDownload` both build on
`read_range`, so each backend implements the minimum. `FileDownload`/`WindowedDownload` keep
reporting the **full** object size (resume-friendly progress), as today.

### Neutral type surface (sized after consumer mapping)

Backend neutrality requires `mtp::` to own these types (today they are `ptp::` types leaked through
re-export or return values). Each gets `From`/`Into` conversions from the `ptp::` equivalents so
`UsbBackend` converts only at its boundary:

- `mtp::ObjectHandle`, `mtp::StorageId` — opaque `Copy` `u64` tokens (see move #2). Keep the `ROOT`
  / `ALL` sentinels consumers rely on.
- `mtp::ObjectInfo` — keep the fields/helpers consumers use (`handle`, `storage_id`, `parent`,
  `filename`, `size`, `created`/`modified`, `is_folder()`/`is_file()`), with a **neutral**
  `format: ObjectFormat` instead of `ptp::ObjectFormatCode`.
- `mtp::ObjectFormat` — neutral format enum (folder/image/audio/video/other + raw code escape
  hatch). WPD exposes MTP format codes, so the mapping is shared.
- `mtp::DeviceInfo` — small neutral struct: `manufacturer`, `model`, `serial_number`,
  `device_version`. Capability/operation lists do **not** belong here (WPD can't fill them).
- `mtp::StorageInfo` — `total_capacity`, `free_space`, `description`, `volume_identifier`,
  `is_writable` (+ neutral `StorageType` if needed). Replaces the leaked `AccessCapability`/
  `StorageType`/`FilesystemType` enums consumers match on.
- `mtp::DateTime` — neutral Y/M/D H:M:S (the `ptp::` one is already backend-neutral in spirit; lift
  it). Cmdr converts it; keep the same fields.

`ptp::` keeps all of the above in their PTP-specific forms for low-level/camera users. The reset path
(`ptp::PtpDevice` + `transport::NusbTransport`, used by the CLI's `reset`) stays public and untouched.

### Consumer blast radius (Phase 1 exit gate)

Both first-party consumers must compile and pass against the new API before Phase 1 lands:

- **`mtp-rs-cli`**: prints `handle.0`/`storage_id.0` in JSON (`output.rs`, command rows) and parses a
  storage id from a string (`device.rs`) — opaque `u64` still prints/parses fine, but the inner
  accessor changes. Matches `Error` + `ResponseCode` variants (`error.rs`), uses `supports_rename()`,
  `download_stream`, `upload_with_progress`, `ObjectInfo` fields, `MtpDeviceInfo`. No `session()` /
  `is_android()` use. Isolated `ptp::`/`transport::` use in `reset.rs` is unaffected.
- **Cmdr** (`apps/desktop/src-tauri`, currently pins published `mtp-rs = "0.22.0"`): ~15 files,
  ~100+ sites. Heaviest: `connection/errors.rs` (full `Error`+`ResponseCode` match — rewrite against
  neutral `Error`), `connection/file_ops.rs` (windowed download + upload + `handle.0` wire
  serialization), handle/id construction across `*_ops.rs`, `ObjectFormatCode::Association` directory
  checks, `AccessCapability::ReadWrite`. No `session()` use. **During Phase 1, repoint Cmdr's
  dependency to a local `path`/branch** to validate, then back to a version on release.

### Cross-process handle stability (note for Phase 2)

The CLI is one process per command, so any id it prints and later accepts (storage selection) must be
**stable across sessions**. For `UsbBackend` the opaque token equals the real PTP id (stable) — no
issue in Phase 1. For `WpdBackend`, derive the token deterministically from the WPD string id (stable
per device) rather than a per-session counter, or have the CLI address by path across invocations.
Resolved in Phase 2; flagged here so the token scheme is chosen with this in mind.

### Streaming types

`FileDownload` and `ObjectListing` become concrete public structs wrapping
`Pin<Box<dyn Stream<Item = Result<_, Error>> + Send>>`, so they are backend-neutral with no per-type
backend enum. UsbBackend feeds them from `ReceiveStream`/the listing loop; WpdBackend feeds them from
a bounded channel driven by the COM worker thread.

### Neutral error mapping (representative, not exhaustive)

`mtp::Error` neutral variants: `NotFound`, `StaleHandle`, `AccessDenied`, `Unsupported`, `Busy`,
`StorageFull`, `Cancelled`, `Disconnected`, `InvalidData`, `Io`, `Protocol`(opaque detail).

PTP `ResponseCode` → neutral:
- `InvalidObjectHandle` / `InvalidParentObject` on a previously-valid handle → `StaleHandle`
  (preserves the Android "re-list parent and retry once" recovery path; see AGENTS.md quirk).
- `StoreReadOnly` / `AccessDenied` → `AccessDenied`
- `DeviceBusy` → `Busy`; `StoreFull` → `StorageFull`
- `OperationNotSupported` / `ParameterNotSupported` → `Unsupported`
- transport/USB faults → `Disconnected` / `Io`

WPD `HRESULT` → neutral:
- `E_ACCESSDENIED` → `AccessDenied`
- `HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND / ERROR_PATH_NOT_FOUND)` → `NotFound`
- `HRESULT_FROM_WIN32(ERROR_BUSY / ERROR_DEVICE_NOT_AVAILABLE)` → `Busy` / `Disconnected`
- `STG_E_MEDIUMFULL` → `StorageFull`
- `E_NOTIMPL` / unsupported command → `Unsupported`
- device removed (`ERROR_DEVICE_REMOVED`) → `Disconnected`

The full tables live with each backend's implementation. `StaleHandle` matters: WPD IDs are more
stable than PTP handles, but re-keying can still occur, so the recovery path stays backend-neutral.

## The WPD backend (COM)

### Threading: a worker-thread actor

WPD COM pointers are `!Send`/`!Sync` and apartment-affine. Rather than fight that with `unsafe`
`Send` wrappers, **one dedicated `std::thread` per open device** calls
`CoInitializeEx(COINIT_MULTITHREADED)`, owns *all* the COM interface pointers, and processes one
request at a time off a channel, replying via oneshots. Consequences:

- `MtpDevice`/`WpdBackend` hold only channel senders → `Send + Sync` with **zero unsafe**.
- One-op-at-a-time serialization falls out for free (matches MTP's model and the existing
  `operation_lock`).
- Streaming downloads: the worker reads `IStream` chunks and pushes `Bytes` into a **bounded** channel
  the `FileDownload` pulls from (backpressure). Dropping the `FileDownload` signals the worker to
  release the `IStream`. No SIC class-cancel needed — WPD cancel is "stop reading + `Release`".
- Lifecycle: worker spawns on `open`, runs until `close()` or `MtpDevice` drop (which sends a shutdown
  message, joins, and `CoUninitialize`s on that thread).

This actor is an implementation detail fully hidden behind `MtpBackend`. The public API stays async
and `Send + Sync` regardless of backend.

### WPD operation mapping

- enumerate / open → `IPortableDeviceManager::GetDevices`, `IPortableDevice::Open` (+ client-info
  `IPortableDeviceValues`).
- device info → `WPD_DEVICE_SERIAL_NUMBER`, manufacturer, friendly name. `speed`/`location_id` are
  `None`/synthesized (the info structs are `#[non_exhaustive]`, so this is free).
- storages → functional objects of category `WPD_FUNCTIONAL_CATEGORY_STORAGE`, or children of the
  `"DEVICE"` root.
- list (+stream +cancel) → `IPortableDeviceContent::EnumObjects` + `IPortableDeviceProperties::
  GetValues` (`WPD_OBJECT_NAME`, `WPD_OBJECT_ORIGINAL_FILE_NAME`, `WPD_OBJECT_SIZE`,
  `WPD_OBJECT_CONTENT_TYPE`/format). `CancelToken` checked between batches.
- download (+range) → `IPortableDeviceResources::GetStream(WPD_RESOURCE_DEFAULT, STGM_READ)`,
  `IStream::Seek` for offset, read loop.
- upload (+progress) → `IPortableDeviceContent::CreateObjectWithPropertiesAndData` → `IStream` write
  → `Commit`. **Transactional**: a failure before `Commit` leaves no object, so the partial-handle
  contract differs from USB (where `SendObjectInfo` creates a handle the data phase can leave
  partial). Document the WPD semantics explicitly; `UploadError::partial` is `None` on WPD.
- delete → `IPortableDeviceContent::Delete`.
- create folder / rename / move / copy → object creation / property writes / WPD content ops.
- events → `IPortableDeviceEventCallback::Advise` (the fiddliest piece; see open decisions).

## Phase 0 findings (proven, load-bearing)

- **Pure-Rust WPD works end to end** on a real Pixel 9 Pro XL: enumerate → open → serial → multi-level
  list → `GetStream` download → byte-count + SHA-256 verified.
- **windows-rs coverage is effectively complete.** Using `windows` 0.62.2,
  `default-features = false`, features: `Win32_Foundation`, `Win32_System_Com`,
  `Win32_System_Com_StructuredStorage`, `Win32_UI_Shell_PropertiesSystem`,
  `Win32_Devices_PortableDevices`. **Zero** missing symbols for every `PROPERTYKEY`/`GUID`/`CLSID`/
  interface needed. **The only hand-defined constant is the root object id string
  `WPD_DEVICE_OBJECT_ID = "DEVICE"`** (a C macro windows-rs does not project).
- **Self-contained artifact:** ~3.34 MB binary, links only system COM DLLs via `raw-dylib`, **no
  Windows SDK and no MinGW runtime DLLs needed to run.** Build footprint: ~103s clean build, ~9.7 MB
  crate downloads, ~110 MB extracted source (108.9 MB is the `windows` umbrella crate alone), ~94 MB
  target.
- **COM ergonomics (windows-rs):** RAII refcounting/`Drop` is automatic; `CoCreateInstance::<T>()` is
  generic. Typed property-bag getters avoid most `PROPVARIANT` handling (`GetStringValue`→`PWSTR`,
  `GetGuidValue`→`GUID`, `GetUnsignedLargeIntegerValue`→`u64`). Returned `PWSTR`s are COM-allocated →
  must `CoTaskMemFree`; UTF-16↔`String` is manual. Signature gotchas:
  `IPortableDeviceManager::GetDevices` takes raw `*mut PWSTR` (pass `null_mut()` to probe count);
  device-name APIs take raw `PWSTR` (`PWSTR::null()` to probe, filled buffer to fetch);
  `IEnumPortableDeviceObjectIDs::Next(&mut [PWSTR], *mut u32)`. `GetStream`/`GetValues`/`IStream::Read`
  matched intuition. **The spike source is the reference skeleton for Phase 2.**

### Toolchain decision (Windows)

Use the **MSVC** toolchain for the real backend. The spike used `x86_64-pc-windows-gnu` to dodge the
admin install, which forced workarounds: the Hungarian username path (`C:\Users\Felhasználó`) breaks
MinGW's ANSI `ld`, requiring relocation to an ASCII `C:\rusttc` tree plus portable binutils. MSVC's
`link.exe` is Unicode-safe and standard (raw-dylib still means no SDK import libs are needed at the
binding level, but MSVC provides a robust linker). One-time install of VS Build Tools removes the
whole house of cards before we run real-device integration tests repeatedly.

## Dependency

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.62", default-features = false, features = [
    "Win32_Foundation", "Win32_System_Com", "Win32_System_Com_StructuredStorage",
    "Win32_UI_Shell_PropertiesSystem", "Win32_Devices_PortableDevices",
] }
```

This is consistent with the project's "no C deps / no `-sys`" principle: `windows` is a pure-Rust
projection (no C/C++, no bundled libraries), and `nusb` already FFIs into Windows DLLs via
`windows-sys 0.61` on Windows today. Decision deferred but cheap: ship with the full `windows` crate
through Phases 1-2 for iteration speed, then optionally slim to `windows-core` (~36 KB) + a committed
`windows-bindgen`-generated WPD slice + `windows-link` (~6 KB) before release if the build cost
annoys. Phase 0 proved the swap is mechanical (coverage is complete; only import paths change). Verify
the latest stable `windows` version is ≥14 days old at add time.

## Testing strategy

Safety is a first-class goal. The centerpiece is a **backend-conformance suite**: one set of
behavioral tests written against the `mtp::` public API, run against *every* backend, giving provable
parity.

- **Existing tests are the regression baseline** and must stay green unchanged in behavior: unit
  (mock transport), virtual-device (`feature = "virtual-device"`), CLI cross-process, proptest
  fuzzing. Because UsbBackend wraps today's logic and virtual/mock ride it, these keep exercising the
  same code.
- **Backend-conformance suite** (new): list/round-trip upload→list→download→verify→delete, ranged/
  resumable download, rename/move/copy, create folder, object info, thumbnails, cancellation
  (cancel() and drop), error mapping (stale handle, not-found, access-denied). Runs:
  - against **UsbBackend + virtual device** on Linux/macOS/Windows in CI (no hardware),
  - against **WpdBackend + real device** on Windows, `#[ignore]`, run locally on David's laptop.
- **Real-device integration (Windows, `#[ignore]`):** the conformance suite plus device-specific
  checks, against a Pixel 9 Pro XL and ideally the Redmi Turbo 3 (the issue #13 device) for vendor
  coverage.
- **Cmdr consumer check:** Cmdr compiles and its test suite passes against the new API (mechanical
  update for renamed types; David owns Cmdr, done in the same effort).
- **CI matrix:** add macOS and Windows runners. Windows CI: build everything incl. `WpdBackend`, run
  unit + virtual-device conformance (UsbBackend path). Real-device tests are local-only (CI has no
  phone).
- **Parity matrix** (kept in this doc or a tasks file): operations × {UsbBackend(virtual),
  UsbBackend(real), WpdBackend(real)} × test status. No silent gaps — anything untested on a backend
  is logged, not assumed-covered.
- **Performance:** extend `benchmarks/` with a Windows WPD-vs-USB throughput note (WPD has its own
  buffering; measure rather than assume).

## Execution phasing

### Phase 0 — spike (DONE)

Hardware-verified above. Preserve the spike source as the Phase 2 reference.

### Phase 1 — backend seam + neutral `mtp::` (Mac)

Introduce `MtpBackend`, move all current high-level logic into `UsbBackend`, and land the full
breaking neutralization in one pass:

- `MtpBackend` trait + `Box<dyn MtpBackend>` façade; `UsbBackend` as sole impl (nusb + virtual +
  mock all ride it).
- Opaque `ObjectHandle`/`StorageId`; consolidate downloads into the `ByteRange` primitive; neutral
  `mtp::DeviceInfo` + `Capabilities`; neutral `mtp::Error` + mapping; drop `session()`; unified
  cancellation contract; `FileDownload`/`ObjectListing` over boxed streams.
- Build the backend-conformance suite; run it against UsbBackend+virtual.
- Update `mtp-rs-cli` and Cmdr for the renamed/neutral API.

**Exit criteria:** full existing suite + conformance suite green on Linux/macOS and Windows-build;
`mtp-rs-cli` and Cmdr compile and pass; demonstrable zero behavior change on USB/virtual/mock
(same code paths). This is the load-bearing, careful phase; no WPD code yet.

### Phase 2 — WpdBackend read path (Windows)

- COM actor (`actor.rs`), `com.rs` wrappers, `ids.rs` bimap, `props.rs`, `consts.rs`.
- enumerate, open, device info, capabilities probe, storages, list (+stream +cancel), download
  (+range), thumbnails, object info.
- Auto backend selection (Windows→WPD) + builder override.
- Run the conformance suite (read subset) against WpdBackend on a real Pixel (+Redmi).

**Exit criteria:** read-side parity matrix green on real hardware; auto-selection works; `Send+Sync`
and async hold with zero unsafe in the public path.

### Phase 3 — WpdBackend write path (Windows)

- upload (+progress, transactional partial-handle semantics), delete, create folder, rename, move,
  copy.
- Document WPD-specific write/transaction semantics in AGENTS.md.

**Exit criteria:** full read+write conformance parity on real hardware.

### Phase 4 — events (optional / deferred)

`IPortableDeviceEventCallback::Advise` → `DeviceEvent` stream. See open decisions.

### Phase 5 — docs, CI, ship

- Fix the wrong README Windows section; update `architecture.md`, `AGENTS.md`, `debugging.md`,
  `docs/notes/community-threads.md`. Update the `winmtp` comparison framing.
- Land the macOS + Windows CI matrix.
- Reply to issue #13 with the resolution.
- Release.

## Risks and mitigations

- **COM threading / `Send`:** mitigated by the actor pattern (feasibility shown in Phase 0; full
  validation in Phase 2).
- **Per-device WPD property variance:** mitigated by the real-device conformance suite + lenient
  parsing (mirror the existing Lumix-style datetime leniency: unparseable → `None`, never fail the
  whole listing).
- **WPD performance vs raw USB:** measure in `benchmarks/`; WPD buffers differently.
- **Breaking-change ripple into Cmdr / CLI:** coordinated and mechanical; David owns both; done in
  Phase 1.
- **Scope creep of neutralization:** bounded to "match current capabilities, add no features."

## Open decisions

1. **Neutral-error rework timing** — recommend doing the full neutral `mtp::Error` in Phase 1 (it is
   the elegant once-and-done moment; the Cmdr ripple is mechanical) rather than as a fast-follow.
2. **Events in v1 or deferred** — recommend deferring to Phase 4 and shipping read+write parity
   first; events are the fiddliest WPD piece and least essential.
