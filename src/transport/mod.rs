//! USB transport abstraction layer.

#[cfg(test)]
pub mod mock;
pub mod nusb;

#[cfg(feature = "virtual-device")]
pub mod virtual_device;

pub use self::nusb::{NusbTransport, UsbDeviceInfo};

use async_trait::async_trait;
use std::time::Duration;

/// Transport trait for MTP/PTP communication.
///
/// Abstracts USB communication to enable testing with mock transport.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send data on the bulk OUT endpoint.
    async fn send_bulk(&self, data: &[u8]) -> Result<(), crate::Error>;

    /// Receive data from the bulk IN endpoint.
    ///
    /// `max_size` is the maximum bytes to receive in one call.
    async fn receive_bulk(&self, max_size: usize) -> Result<Vec<u8>, crate::Error>;

    /// Receive event data from the interrupt IN endpoint.
    ///
    /// This may block until an event is available.
    async fn receive_interrupt(&self) -> Result<Vec<u8>, crate::Error>;

    /// Cancel an in-progress transfer using the USB Still Image Class mechanism.
    ///
    /// Sends a CLASS_CANCEL control request (`bRequest=0x64`), then drains
    /// remaining data from the bulk IN and interrupt pipes. After this call,
    /// the session is clean and ready for the next operation.
    ///
    /// `idle_timeout` controls how long to wait during drain steps before
    /// assuming each pipe is clear. 300ms (matching libmtp / Windows) is
    /// the recommended default; see [`mtp::DEFAULT_CANCEL_TIMEOUT`](crate::mtp::DEFAULT_CANCEL_TIMEOUT).
    async fn cancel_transfer(
        &self,
        transaction_id: u32,
        idle_timeout: Duration,
    ) -> Result<(), crate::Error>;
}
