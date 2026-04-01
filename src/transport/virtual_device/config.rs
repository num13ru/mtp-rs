//! Configuration types for virtual MTP devices.

use std::path::PathBuf;
use std::time::Duration;

/// Configuration for a virtual MTP device.
///
/// Defines the identity and storages of a virtual device that operates against
/// a local filesystem directory instead of real USB hardware.
///
/// # Example
///
/// ```rust
/// use std::path::PathBuf;
/// use std::time::Duration;
/// use mtp_rs::transport::virtual_device::config::{VirtualDeviceConfig, VirtualStorageConfig};
///
/// let config = VirtualDeviceConfig {
///     manufacturer: "Google".into(),
///     model: "Virtual Pixel 9".into(),
///     serial: "virtual-001".into(),
///     storages: vec![VirtualStorageConfig {
///         description: "Internal Storage".into(),
///         capacity: 64 * 1024 * 1024 * 1024,
///         backing_dir: PathBuf::from("/tmp/mtp-test"),
///         read_only: false,
///     }],
///     supports_rename: true,
///     event_poll_interval: Duration::from_millis(50),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct VirtualDeviceConfig {
    /// Manufacturer name reported by the virtual device.
    pub manufacturer: String,
    /// Model name reported by the virtual device.
    pub model: String,
    /// Serial number for the virtual device.
    pub serial: String,
    /// Storage configurations. At least one storage is required.
    pub storages: Vec<VirtualStorageConfig>,
    /// Whether the device advertises SetObjectPropValue support (rename).
    pub supports_rename: bool,
    /// How long `receive_interrupt` waits when no events are pending.
    /// Simulates the USB interrupt endpoint blocking behavior.
    /// Default: 50ms for production use. Use `Duration::ZERO` in tests
    /// to avoid slowing down the test suite.
    pub event_poll_interval: Duration,
}

/// Configuration for a single storage within a virtual device.
#[derive(Debug, Clone)]
pub struct VirtualStorageConfig {
    /// Human-readable storage description (for example, "Internal Storage").
    pub description: String,
    /// Maximum storage capacity in bytes.
    pub capacity: u64,
    /// Local directory backing this storage. Files here become MTP objects.
    pub backing_dir: PathBuf,
    /// If true, write operations return `StoreReadOnly`.
    pub read_only: bool,
}
