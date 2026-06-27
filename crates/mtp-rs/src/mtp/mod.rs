//! High-level MTP (Media Transfer Protocol) API for Android devices and media players.
//!
//! This module provides a convenient, batteries-included API for common file transfer
//! operations. Use this module when:
//!
//! - Working with Android phones and tablets
//! - You want simple file listing, upload, and download
//! - You need storage enumeration and device info
//! - You don't need camera-specific features (capture, live view, etc.)
//!
//! ## When to use `ptp` instead
//!
//! Use the lower-level [`crate::ptp`] module when you need:
//! - Direct control over PTP operations and transactions
//! - Camera-specific functionality
//! - Custom protocol extensions
//! - Access to raw response codes and error details
//!
//! ## Quick example
//!
//! ```rust,no_run
//! use mtp_rs::mtp::MtpDevice;
//!
//! # async fn example() -> Result<(), mtp_rs::Error> {
//! let device = MtpDevice::open_first().await?;
//! for storage in device.storages().await? {
//!     for obj in storage.list_objects(None).await? {
//!         println!("{}", obj.filename);
//!     }
//! }
//! # Ok(())
//! # }
//! ```

mod device;
mod event;
mod object;
mod storage;
mod stream;
mod types;

pub use device::{MtpDevice, MtpDeviceBuilder, MtpDeviceInfo};
pub use event::DeviceEvent;
// Backend-neutral high-level types (see types.rs). Not yet wired into the public method signatures
// — that happens in the façade step; defined first so the backend trait can speak this vocabulary.
pub use types::{
    Capabilities, DateTime, DeviceInfo, FilesystemType, ObjectFormat, ObjectHandle, ObjectInfo,
    StorageId, StorageInfo, StorageType,
};
pub use object::NewObjectInfo;
pub use storage::{ObjectListing, Storage};
pub use stream::{
    FileDownload, Progress, WindowedDownload, DEFAULT_CANCEL_TIMEOUT, DEFAULT_DOWNLOAD_WINDOW,
};
