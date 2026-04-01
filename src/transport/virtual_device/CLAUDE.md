# Virtual device transport

Feature-gated (`virtual-device`) Transport implementation backed by local filesystem directories instead of USB. Exercises the full MTP/PTP binary protocol path.

## Architecture

```
MtpDevice (unchanged)
  → MtpDeviceInner (unchanged)
    → PtpSession (unchanged)
      → Arc<dyn Transport>
        → VirtualTransport — implements Transport trait
          → VirtualDeviceState — in-memory object tree + filesystem
```

## Module structure

- `config.rs` — `VirtualDeviceConfig`, `VirtualStorageConfig` (public types)
- `state.rs` — `VirtualDeviceState`, `VirtualObject`, `PendingCommand`, handle management
- `builders.rs` — binary payload builders (DeviceInfo, StorageInfo, ObjectInfo, containers)
- `handlers.rs` — protocol operation handlers dispatched by opcode
- `registry.rs` — global virtual device registry for discovery integration (`list_devices`, `open_by_location`, `open_by_serial`)
- `mod.rs` — `VirtualTransport` struct + `Transport` impl + tests

## Key decisions

- **`std::sync::Mutex`** over `parking_lot::Mutex`: `parking_lot` is only a dev-dep. Virtual transport isn't performance-critical, so std mutex is fine.
- **`PendingCommand` struct**: When the host sends a command that expects a data phase (SendObjectInfo, SendObject, SetObjectPropValue), the command is stored as a `PendingCommand` in `state.pending_command`. The next `send_bulk` (data container) takes it via `.take()` and dispatches both together. This keeps pending state separate from the response queue.
- **`VecDeque` for queues**: `response_queue` and `event_queue` use `VecDeque` for O(1) front removal (FIFO access pattern).
- **Discovery via global registry**: Virtual devices can be registered via `register_virtual_device()` to appear in `MtpDevice::list_devices()`. They get synthetic location IDs starting at `0xFFFF_0000_0000_0000` to avoid collisions with real USB devices. Uses `OnceLock` for the static registry (MSRV 1.79).
- **Event poll interval**: `VirtualTransport` stores `event_poll_interval: Duration` outside the mutex. When no events are pending, `receive_interrupt` awaits this delay before returning `Timeout`, preventing CPU spin in event loops. Tests use `Duration::ZERO` for speed; production callers should use 50ms+.

## Gotchas

- `list_objects(None)` applies a parent filter (`ParentFilter::Exact(ROOT)`), so the virtual transport must set `parent = ObjectHandle::ROOT` on root-level objects for them to appear.
- `SendObjectInfo` for folders creates the directory immediately (no `SendObject` phase needed for folders per MTP spec).
- Storage IDs start at `0x00010001` (matching real MTP convention).
- The global registry is process-wide and shared across tests. Registry tests must clean up with `unregister_virtual_device()` and use unique serial numbers to avoid interference.
- `event_poll_interval` lives on `VirtualTransport` (not inside `VirtualDeviceState`) because we need it after dropping the mutex lock and before an async `.await`.
