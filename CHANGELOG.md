# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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

## [0.1.0] - Unreleased

Initial release targeting modern Android devices.

### Features

- Connect to Android phones/tablets over USB
- List, download, upload, delete, move, and copy files
- Create and delete folders
- Stream large file downloads with progress tracking
- Listen for device events (file added, storage removed, etc.)
- Two-layer API: high-level `mtp::` and low-level `ptp::`
- Runtime-agnostic async design (works with tokio, async-std, etc.)
- Pure Rust implementation using `nusb` for USB access

### Not included (by design)

- MTPZ (DRM extension for old devices)
- Playlist and metadata syncing
- Vendor-specific extensions
- Legacy device quirks database
