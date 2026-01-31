# Contributing to mtp-rs

Thanks for considering contributing! This document covers the practical stuff you need to know.

## Getting started

```bash
git clone https://github.com/vdavid/mtp-rs
cd mtp-rs
cargo build
cargo test
```

You don't need an MTP device for most development. The test suite uses mock transports for protocol logic.

## Project structure

```
src/
├── ptp/             # Low-level protocol implementation
│   ├── codes.rs     # Operation/response/event code enums
│   ├── pack.rs      # Binary serialization (little-endian, UTF-16LE strings)
│   ├── container.rs # USB container format
│   ├── types.rs     # DeviceInfo, StorageInfo, ObjectInfo structs
│   ├── session.rs   # Session management and operations
│   └── device.rs    # PtpDevice public API
├── mtp/             # High-level API
│   ├── device.rs    # MtpDevice and builder
│   ├── storage.rs   # Storage and file operations
│   ├── stream.rs    # Streaming downloads
│   ├── object.rs    # NewObjectInfo for uploads
│   └── event.rs     # Device events
├── transport/       # USB abstraction
│   ├── mock.rs      # Mock for testing
│   └── nusb.rs      # Real USB implementation
├── error.rs         # Error types
└── lib.rs           # Crate root

tests/
└── integration.rs   # Tests that need a real device
```

## Running tests

```bash
# Unit tests (no device needed)
cargo test

# With a real Android device connected
cargo test --test integration -- --ignored --nocapture
```

The integration tests are split into read-only (safe) and destructive (creates/deletes files) to avoid messing up
your phone if you don't trust the lib too much but still want to run some tests.

Integration tests run serially to avoid the obvious collisions.

## Code style

We follow standard Rust conventions:

- `cargo fmt` before committing
- `cargo clippy` should pass with no warnings
- Tests for new functionality
- Doc comments for public APIs

No need to over-document internal code. If the code is clear, a brief comment or none at all is fine.

## Architecture decisions

A few things that might not be obvious:

- **Two-layer API**: The `ptp::` module is the protocol implementation, `mtp::` is the user-friendly wrapper. Most
  changes to user-facing behavior go in `mtp::`, protocol fixes go in `ptp::`.
- **Runtime agnostic**: We don't depend on tokio directly. Use `futures` traits and `futures-timer` for timeouts. This
  lets users bring their own runtime.
- **No device quirks**: Unlike libmtp, we don't have a quirks database. Modern Android devices all behave the same way.
  If you find a device that doesn't work, let's understand why before adding workarounds.
- **Mock transport for testing**: `transport::mock::MockTransport` lets you test protocol logic without USB. Queue
  expected responses and verify sent commands.

## What we're looking for

- Testing with real devices and updating [README.md](README.md#tested-devices) with more device info
- Bug reports (with reproduction steps)
- Docs improvements
- Maybe some more PTP implementations like battery level, I'm unsure if we really want it or we should focus on MTP

We're not really looking to add legacy stuff like:

- MTPZ support
- Legacy device quirks
- Playlist/metadata syncing
- Vendor extensions

These don't seem that useful.

## The protocol

If you need to understand MTP/PTP, see the docs:

- [`docs/protocol.md`](docs/protocol.md) - Wire format, operations, data structures
- [`docs/architecture.md`](docs/architecture.md) - Module structure and design decisions
- [`docs/debugging.md`](docs/debugging.md) - USB capture for troubleshooting
- [`mtp-v1_1-spec-md`](https://github.com/vdavid/mtp-v1_1-spec-md) - Separate repo. The full MTP spec. Reference only, it's dense.

The protocol is essentially:

1. Send a command container (operation code + params)
2. Optionally send/receive data containers
3. Receive a response container (success/error code + params)

Everything is little-endian. Strings are UTF-16LE with a length prefix.

## Submitting changes

1. Fork and create a branch
2. Make your changes
3. Run `cargo fmt` and `cargo clippy`
4. Run `cargo test`
5. If you have a device, run integration tests
6. Open a PR with a clear description incl. how you tested your changes

For non-trivial changes, consider opening an issue first to discuss the approach.

## Questions?

Open an issue. We're happy to chat!
