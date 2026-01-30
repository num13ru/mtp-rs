//! USB transport implementation using nusb.

use super::Transport;
use async_trait::async_trait;
use nusb::transfer::{Direction, EndpointType, RequestBuffer, TransferError};
use std::time::Duration;

/// MTP interface class code (Still Image).
const MTP_CLASS_IMAGE: u8 = 0x06;
/// MTP interface class code (Vendor-specific).
const MTP_CLASS_VENDOR: u8 = 0xFF;
/// MTP subclass code.
const MTP_SUBCLASS: u8 = 0x01;
/// MTP protocol code (PTP).
const MTP_PROTOCOL: u8 = 0x01;

/// USB transport implementation using nusb.
pub struct NusbTransport {
    interface: nusb::Interface,
    bulk_in: u8,
    bulk_out: u8,
    interrupt_in: u8,
    timeout: Duration,
}

impl NusbTransport {
    /// Default timeout for USB operations (30 seconds for large transfers).
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

    /// Default buffer size for interrupt transfers.
    const INTERRUPT_BUFFER_SIZE: usize = 64;

    /// List all available MTP devices.
    pub fn list_mtp_devices() -> Result<Vec<nusb::DeviceInfo>, crate::Error> {
        let devices = nusb::list_devices()
            .map_err(crate::Error::Usb)?
            .filter(Self::is_mtp_device)
            .collect();
        Ok(devices)
    }

    /// Check if a device info represents an MTP device.
    fn is_mtp_device(dev: &nusb::DeviceInfo) -> bool {
        // Check device class/subclass/protocol at device level
        if Self::is_mtp_class(dev.class(), dev.subclass(), dev.protocol()) {
            return true;
        }

        // Many Android devices use class 0 at device level
        // and have the MTP interface at the interface level.
        // We need to open the device to check interface descriptors,
        // but for listing we can check the device-level class.
        // If class is 0 (composite device), we optimistically include it
        // and will verify when opening.
        if dev.class() == 0 {
            // This might be a composite device with MTP interface
            // We could try to open and check, but for listing purposes
            // we'll be conservative and not include unknown devices.
            // The user can always explicitly open a device.
            return false;
        }

        false
    }

    /// Check if class/subclass/protocol match MTP.
    fn is_mtp_class(class: u8, subclass: u8, protocol: u8) -> bool {
        (class == MTP_CLASS_IMAGE || class == MTP_CLASS_VENDOR)
            && subclass == MTP_SUBCLASS
            && protocol == MTP_PROTOCOL
    }

    /// Open a specific device and claim the MTP interface.
    pub async fn open(device: nusb::Device) -> Result<Self, crate::Error> {
        Self::open_with_timeout(device, Self::DEFAULT_TIMEOUT).await
    }

    /// Open with custom timeout.
    pub async fn open_with_timeout(
        device: nusb::Device,
        timeout: Duration,
    ) -> Result<Self, crate::Error> {
        // Find the MTP interface
        let config = device
            .active_configuration()
            .map_err(|e| crate::Error::invalid_data(format!("Failed to get configuration: {}", e)))?;

        let mut mtp_interface_number = None;
        let mut bulk_in = None;
        let mut bulk_out = None;
        let mut interrupt_in = None;

        for interface in config.interfaces() {
            // Get the first alternate setting for this interface
            let Some(alt_setting) = interface.alt_settings().next() else {
                continue;
            };

            // Check if this interface is MTP
            if Self::is_mtp_class(
                alt_setting.class(),
                alt_setting.subclass(),
                alt_setting.protocol(),
            ) {
                mtp_interface_number = Some(interface.interface_number());

                // Find endpoints
                for endpoint in alt_setting.endpoints() {
                    match (endpoint.direction(), endpoint.transfer_type()) {
                        (Direction::Out, EndpointType::Bulk) => {
                            bulk_out = Some(endpoint.address());
                        }
                        (Direction::In, EndpointType::Bulk) => {
                            bulk_in = Some(endpoint.address());
                        }
                        (Direction::In, EndpointType::Interrupt) => {
                            interrupt_in = Some(endpoint.address());
                        }
                        _ => {}
                    }
                }

                break;
            }
        }

        let interface_number = mtp_interface_number
            .ok_or_else(|| crate::Error::invalid_data("No MTP interface found on device"))?;

        let bulk_in =
            bulk_in.ok_or_else(|| crate::Error::invalid_data("No bulk IN endpoint found"))?;
        let bulk_out =
            bulk_out.ok_or_else(|| crate::Error::invalid_data("No bulk OUT endpoint found"))?;
        let interrupt_in = interrupt_in
            .ok_or_else(|| crate::Error::invalid_data("No interrupt IN endpoint found"))?;

        // Claim the interface
        let interface = device
            .claim_interface(interface_number)
            .map_err(crate::Error::Usb)?;

        Ok(Self {
            interface,
            bulk_in,
            bulk_out,
            interrupt_in,
            timeout,
        })
    }

    /// Get the timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Set the timeout.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Get the bulk IN endpoint address.
    pub fn bulk_in_endpoint(&self) -> u8 {
        self.bulk_in
    }

    /// Get the bulk OUT endpoint address.
    pub fn bulk_out_endpoint(&self) -> u8 {
        self.bulk_out
    }

    /// Get the interrupt IN endpoint address.
    pub fn interrupt_in_endpoint(&self) -> u8 {
        self.interrupt_in
    }

    /// Convert a nusb TransferError to crate::Error.
    fn convert_transfer_error(err: TransferError) -> crate::Error {
        match err {
            TransferError::Cancelled => crate::Error::Cancelled,
            TransferError::Disconnected => crate::Error::Disconnected,
            TransferError::Stall | TransferError::Fault | TransferError::Unknown => {
                crate::Error::Usb(std::io::Error::other(err.to_string()))
            }
        }
    }
}

#[async_trait]
impl Transport for NusbTransport {
    async fn send_bulk(&self, data: &[u8]) -> Result<(), crate::Error> {
        let result = futures::future::select(
            Box::pin(self.interface.bulk_out(self.bulk_out, data.to_vec())),
            Box::pin(futures_timer::Delay::new(self.timeout)),
        )
        .await;

        match result {
            futures::future::Either::Left((completion, _)) => {
                completion.status.map_err(Self::convert_transfer_error)?;
                Ok(())
            }
            futures::future::Either::Right((_, _)) => Err(crate::Error::Timeout),
        }
    }

    async fn receive_bulk(&self, max_size: usize) -> Result<Vec<u8>, crate::Error> {
        let result = futures::future::select(
            Box::pin(
                self.interface
                    .bulk_in(self.bulk_in, RequestBuffer::new(max_size)),
            ),
            Box::pin(futures_timer::Delay::new(self.timeout)),
        )
        .await;

        match result {
            futures::future::Either::Left((completion, _)) => {
                completion.status.map_err(Self::convert_transfer_error)?;
                Ok(completion.data)
            }
            futures::future::Either::Right((_, _)) => Err(crate::Error::Timeout),
        }
    }

    async fn receive_interrupt(&self) -> Result<Vec<u8>, crate::Error> {
        let result = futures::future::select(
            Box::pin(self.interface.interrupt_in(
                self.interrupt_in,
                RequestBuffer::new(Self::INTERRUPT_BUFFER_SIZE),
            )),
            Box::pin(futures_timer::Delay::new(self.timeout)),
        )
        .await;

        match result {
            futures::future::Either::Left((completion, _)) => {
                completion.status.map_err(Self::convert_transfer_error)?;
                Ok(completion.data)
            }
            futures::future::Either::Right((_, _)) => Err(crate::Error::Timeout),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires real MTP device
    fn test_list_devices() {
        let devices = NusbTransport::list_mtp_devices().unwrap();
        println!("Found {} MTP devices", devices.len());
        for dev in &devices {
            println!(
                "  {:04x}:{:04x} at {}:{}",
                dev.vendor_id(),
                dev.product_id(),
                dev.bus_number(),
                dev.device_address()
            );
        }
    }

    #[tokio::test]
    #[ignore] // Requires real MTP device
    async fn test_open_device() {
        let devices = NusbTransport::list_mtp_devices().unwrap();
        assert!(!devices.is_empty(), "No MTP device found");

        let device = devices[0].open().unwrap();
        let transport = NusbTransport::open(device).await.unwrap();

        assert_eq!(transport.timeout(), NusbTransport::DEFAULT_TIMEOUT);
    }

    #[tokio::test]
    #[ignore] // Requires real MTP device
    async fn test_timeout_configuration() {
        let devices = NusbTransport::list_mtp_devices().unwrap();
        assert!(!devices.is_empty(), "No MTP device found");

        let device = devices[0].open().unwrap();
        let custom_timeout = Duration::from_secs(60);
        let mut transport = NusbTransport::open_with_timeout(device, custom_timeout)
            .await
            .unwrap();

        assert_eq!(transport.timeout(), custom_timeout);

        let new_timeout = Duration::from_secs(10);
        transport.set_timeout(new_timeout);
        assert_eq!(transport.timeout(), new_timeout);
    }

    #[test]
    fn test_mtp_class_detection() {
        // Image class with MTP subclass/protocol
        assert!(NusbTransport::is_mtp_class(0x06, 0x01, 0x01));

        // Vendor class with MTP subclass/protocol
        assert!(NusbTransport::is_mtp_class(0xFF, 0x01, 0x01));

        // Wrong class
        assert!(!NusbTransport::is_mtp_class(0x08, 0x01, 0x01));

        // Wrong subclass
        assert!(!NusbTransport::is_mtp_class(0x06, 0x00, 0x01));

        // Wrong protocol
        assert!(!NusbTransport::is_mtp_class(0x06, 0x01, 0x00));
    }
}
