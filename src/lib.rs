//! # mtp-rs
//!
//! A pure-Rust MTP (Media Transfer Protocol) library targeting modern Android devices.
//!
//! ## Features
//!
//! - **Runtime agnostic**: Uses `futures` traits, works with any async runtime
//! - **Two-level API**: High-level `mtp::` for media devices, low-level `ptp::` for cameras
//! - **Streaming**: Memory-efficient streaming downloads with progress tracking
//! - **Type safe**: Newtype wrappers prevent mixing up IDs
//!
//! ## Quick start
//!
//! ```rust,ignore
//! use mtp_rs::mtp::MtpDevice;
//!
//! # async fn example() -> Result<(), mtp_rs::Error> {
//! // Open the first MTP device
//! let device = MtpDevice::open_first().await?;
//!
//! println!("Connected to: {} {}",
//!          device.device_info().manufacturer,
//!          device.device_info().model);
//!
//! // Get storages
//! for storage in device.storages().await? {
//!     println!("Storage: {} ({} free)",
//!              storage.info().description,
//!              storage.info().free_space_bytes);
//!
//!     // List root folder
//!     for obj in storage.list_objects(None).await? {
//!         let kind = if obj.is_folder() { "DIR " } else { "FILE" };
//!         println!("  {} {} ({} bytes)", kind, obj.filename, obj.size);
//!     }
//! }
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod ptp;
// pub mod mtp;      // Phase 4
// pub mod transport; // Phase 2-3

pub use error::Error;

// Re-export core types for convenience
pub use ptp::{
    DateTime, EventCode, ObjectFormatCode, ObjectHandle, OperationCode, ResponseCode, SessionId,
    StorageId, TransactionId,
};
