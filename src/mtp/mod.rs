//! High-level MTP (Media Transfer Protocol) API.
//!
//! This module provides a user-friendly API for interacting with MTP devices
//! like Android phones and tablets.

mod device;
mod event;
mod object;
mod storage;
mod stream;

pub use device::{MtpDevice, MtpDeviceBuilder, MtpDeviceInfo};
pub use event::DeviceEvent;
pub use object::NewObjectInfo;
pub use storage::Storage;
pub use stream::{DownloadChunk, DownloadStream, Progress};
