# mtp-rs

A pure-Rust MTP/PTP library with no C dependencies. It provides a two-layer API: `mtp::` for high-level file transfer
operations, and `ptp::` for low-level protocol access (useful for cameras). The key value proposition is zero FFI
dependencies. no libmtp, no libusb. Just async Rust built on `nusb`, making cross-compilation straightforward.

## Quick Commands

| Command                     | Description                                              |
|-----------------------------|----------------------------------------------------------|
| `just`                      | Run all checks: format, lint, test, doc (~1s cached)     |
| `just fix`                  | Auto-fix formatting and clippy warnings (~3s)            |
| `just check-all`            | Include security audit and license check (~4s)           |
| `cargo test`                | Run unit tests                                           |
| `cargo test --all-features` | Run with all features (enables proptest)                 |

## Project Structure

```
src/
  mtp/         # High-level file transfer API (MtpDevice, Storage, DownloadStream)
  ptp/         # Low-level protocol implementation (PtpDevice, PtpSession, containers)
  transport/   # USB transport abstraction (Transport trait, nusb impl, mock for tests)
  error.rs     # Error types
  lib.rs       # Crate root

tests/
  integration.rs   # Integration tests (require real MTP device)

examples/         # Usage examples

docs/
  architecture.md  # Module structure and design decisions
  protocol.md      # Wire format and operations
  debugging.md     # USB capture for troubleshooting
```

## Testing

- **Unit tests**: `cargo test` (no device needed, uses mock transport)
- **Integration tests**: `cargo test --test integration -- --ignored --nocapture` (needs real MTP device)
- **Property tests**: `cargo test --all-features` (enables proptest for fuzzing pack/unpack)

Integration tests are split into read-only (safe) and destructive (creates/deletes files). They run serially to avoid
collisions.

## Architecture Reference

Full details in [docs/architecture.md](docs/architecture.md).

**Layer diagram:**

```
User Application
       |
       v
  mtp:: (MtpDevice, Storage)    <-- Most users start here
       |
       v
  ptp:: (PtpSession, Container)
       |
       v
  transport:: (Transport trait)
       |
       v
  nusb crate (USB access)
```

**Key types:**

- `MtpDevice` - Connect to devices, get info, list storages
- `Storage` - File operations (list, download, upload, delete, move, copy)
- `PtpDevice` / `PtpSession` - Raw protocol access for cameras or debugging
- `ObjectHandle`, `StorageId` - Type-safe handles (newtypes over u32)

## Design Principles

- **Pure Rust**: No C/FFI dependencies. No libusb, no libmtp, no `-sys` crates.
- **Runtime-agnostic**: Uses `futures` traits. Works with tokio, async-std, or any runtime.
- **Modern focus**: Targets well-behaved Android devices (5.0+), not legacy MP3 players.
- **Type-safe handles**: `ObjectHandle(u32)`, `StorageId(u32)` prevent mixing up IDs.
- **Stream-based transfers**: Downloads return `Stream<Item = Chunk>` for memory efficiency and progress tracking.
- **Transport abstraction**: `Transport` trait enables unit testing with mock transport.

## Things to Avoid

When working on this codebase, do NOT:

- **Add C dependencies** (libusb, libmtp, any `-sys` crate). This is a core principle.
- **Add a device quirks database**. Modern Android devices behave consistently. If something breaks, understand why
  first.
- **Implement MTPZ** (the DRM extension). Not useful for modern devices.
- **Add vendor-specific extensions**. Keep the library generic.
- **Add playlist/metadata sync features**. This is a file transfer library, not a media sync tool.
- **Add legacy device workarounds** (pre-Android 5.0 quirks).
- **Make the API surface larger than needed**. Prefer small, focused APIs.
- **Add runtime dependencies** (tokio, async-std). Use `futures` traits only.

## Code Style

See [docs/style-guide.md](docs/style-guide.md) for detailed conventions.

- Run `just check` (or `just fix`) before committing
- `cargo fmt` for formatting
- `cargo clippy` must pass with no warnings (`-D warnings`)
- Tests for new functionality
- Doc comments for public APIs
- Internal code: clarity over comments; if the code is clear, minimal comments are fine

## Useful References

- [docs/architecture.md](docs/architecture.md) - Module structure, dependency rules, design decisions
- [docs/protocol.md](docs/protocol.md) - MTP/PTP wire format, operations, data structures
- [docs/debugging.md](docs/debugging.md) - USB capture and troubleshooting
- [MTP v1.1 Spec (Markdown)](https://github.com/vdavid/mtp-v1_1-spec-md) - Full spec reference
