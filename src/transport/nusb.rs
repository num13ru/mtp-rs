//! USB transport implementation using nusb.

use super::Transport;
use async_trait::async_trait;
use nusb::descriptors::TransferType;
use nusb::transfer::{Buffer, Bulk, Direction, In, Interrupt, Out, TransferError};
use nusb::MaybeFuture;
use std::sync::Mutex;
use std::time::Duration;

/// MTP interface class code (Still Image).
const MTP_CLASS_IMAGE: u8 = 0x06;
/// MTP interface class code (Vendor-specific).
const MTP_CLASS_VENDOR: u8 = 0xFF;
/// MTP subclass code.
const MTP_SUBCLASS: u8 = 0x01;
/// MTP protocol code (PTP).
const MTP_PROTOCOL: u8 = 0x01;

/// USB device information with topology-based location ID.
#[derive(Debug, Clone)]
pub struct UsbDeviceInfo {
    /// USB vendor ID
    pub vendor_id: u16,
    /// USB product ID
    pub product_id: u16,
    /// Manufacturer name (e.g., "Google", "Samsung")
    pub manufacturer: Option<String>,
    /// Product name (e.g., "Pixel 9 Pro XL")
    pub product: Option<String>,
    /// Device serial number (if available)
    pub serial_number: Option<String>,
    /// USB location identifier derived from bus and port topology (stable per port)
    pub location_id: u64,
    /// Reference to the underlying nusb device info for opening
    nusb_info: nusb::DeviceInfo,
}

impl UsbDeviceInfo {
    /// Open the USB device.
    pub fn open(&self) -> Result<nusb::Device, nusb::Error> {
        self.nusb_info.open().wait()
    }
}

/// USB transport implementation using nusb.
pub struct NusbTransport {
    bulk_in: Mutex<nusb::Endpoint<Bulk, In>>,
    bulk_out: Mutex<nusb::Endpoint<Bulk, Out>>,
    interrupt_in: Mutex<nusb::Endpoint<Interrupt, In>>,
    /// Timeout for bulk transfers (sending commands, receiving data).
    timeout: Duration,
    /// Timeout for event polling on the interrupt endpoint.
    event_timeout: Duration,
}

impl NusbTransport {
    /// Default timeout for bulk transfers (30 seconds for large file transfers).
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

    /// Default timeout for event polling (200ms for responsive UI).
    ///
    /// This timeout is used for `receive_interrupt()` which polls for device events.
    /// A short timeout allows callers to poll frequently without blocking other operations.
    /// Adjust via `set_event_timeout()` if you need different responsiveness.
    pub const DEFAULT_EVENT_TIMEOUT: Duration = Duration::from_millis(200);

    /// Default buffer size for interrupt transfers.
    const INTERRUPT_BUFFER_SIZE: usize = 64;

    /// List all available MTP devices with location IDs.
    pub fn list_mtp_devices() -> Result<Vec<UsbDeviceInfo>, crate::Error> {
        let devices = nusb::list_devices()
            .wait()
            .map_err(crate::Error::Usb)?
            .filter(Self::is_mtp_device)
            .map(|dev| {
                let location_id = location_id_from_topology(&dev);
                UsbDeviceInfo {
                    vendor_id: dev.vendor_id(),
                    product_id: dev.product_id(),
                    manufacturer: dev.manufacturer_string().map(String::from),
                    product: dev.product_string().map(String::from),
                    serial_number: dev.serial_number().map(String::from),
                    location_id,
                    nusb_info: dev,
                }
            })
            .collect();
        Ok(devices)
    }

    /// Check if a device info represents an MTP device.
    fn is_mtp_device(dev: &nusb::DeviceInfo) -> bool {
        // Check device class/subclass/protocol at device level
        if Self::is_mtp_class(dev.class(), dev.subclass(), dev.protocol()) {
            return true;
        }

        // Many Android devices are composite (class 0) with MTP as one interface.
        // Check interface-level class info available from DeviceInfo without opening.
        if dev.class() == 0 {
            for intf in dev.interfaces() {
                if Self::is_mtp_class(intf.class(), intf.subclass(), intf.protocol()) {
                    return true;
                }
            }

            // Fall back to opening the device and inspecting configuration descriptors.
            if let Ok(device) = dev.open().wait() {
                if let Ok(config) = device.active_configuration() {
                    for interface in config.interfaces() {
                        if let Some(alt) = interface.alt_settings().next() {
                            if Self::is_mtp_class(alt.class(), alt.subclass(), alt.protocol()) {
                                return true;
                            }
                        }
                    }
                }
            }
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
        Self::open_with_timeouts(device, Self::DEFAULT_TIMEOUT, Self::DEFAULT_EVENT_TIMEOUT).await
    }

    /// Open with custom bulk transfer timeout.
    ///
    /// Uses the default event timeout. For full control over both timeouts,
    /// use `open_with_timeouts()`.
    pub async fn open_with_timeout(
        device: nusb::Device,
        timeout: Duration,
    ) -> Result<Self, crate::Error> {
        Self::open_with_timeouts(device, timeout, Self::DEFAULT_EVENT_TIMEOUT).await
    }

    /// Open with custom timeouts for both bulk transfers and event polling.
    ///
    /// # Arguments
    ///
    /// * `device` - The USB device to open
    /// * `timeout` - Timeout for bulk transfers (commands, file data). Use 30+ seconds
    ///   for large file operations.
    /// * `event_timeout` - Timeout for event polling via `receive_interrupt()`. Use
    ///   100-500ms for responsive event loops without blocking other operations.
    pub async fn open_with_timeouts(
        device: nusb::Device,
        timeout: Duration,
        event_timeout: Duration,
    ) -> Result<Self, crate::Error> {
        // Find the MTP interface
        let config = device.active_configuration().map_err(|e| {
            crate::Error::invalid_data(format!("Failed to get configuration: {}", e))
        })?;

        let mut mtp_interface_number = None;
        let mut bulk_in_addr = None;
        let mut bulk_out_addr = None;
        let mut interrupt_in_addr = None;

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
                        (Direction::Out, TransferType::Bulk) => {
                            bulk_out_addr = Some(endpoint.address());
                        }
                        (Direction::In, TransferType::Bulk) => {
                            bulk_in_addr = Some(endpoint.address());
                        }
                        (Direction::In, TransferType::Interrupt) => {
                            interrupt_in_addr = Some(endpoint.address());
                        }
                        _ => {}
                    }
                }

                break;
            }
        }

        let interface_number = mtp_interface_number
            .ok_or_else(|| crate::Error::invalid_data("No MTP interface found on device"))?;

        let bulk_in_addr =
            bulk_in_addr.ok_or_else(|| crate::Error::invalid_data("No bulk IN endpoint found"))?;
        let bulk_out_addr = bulk_out_addr
            .ok_or_else(|| crate::Error::invalid_data("No bulk OUT endpoint found"))?;
        let interrupt_in_addr = interrupt_in_addr
            .ok_or_else(|| crate::Error::invalid_data("No interrupt IN endpoint found"))?;

        // Claim the interface
        let interface = device
            .claim_interface(interface_number)
            .wait()
            .map_err(crate::Error::Usb)?;

        // Open endpoints
        let bulk_in = interface
            .endpoint::<Bulk, In>(bulk_in_addr)
            .map_err(crate::Error::Usb)?;
        let bulk_out = interface
            .endpoint::<Bulk, Out>(bulk_out_addr)
            .map_err(crate::Error::Usb)?;
        let interrupt_in = interface
            .endpoint::<Interrupt, In>(interrupt_in_addr)
            .map_err(crate::Error::Usb)?;

        Ok(Self {
            bulk_in: Mutex::new(bulk_in),
            bulk_out: Mutex::new(bulk_out),
            interrupt_in: Mutex::new(interrupt_in),
            timeout,
            event_timeout,
        })
    }

    /// Get the bulk transfer timeout.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Set the bulk transfer timeout.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Get the event polling timeout.
    #[must_use]
    pub fn event_timeout(&self) -> Duration {
        self.event_timeout
    }

    /// Set the event polling timeout.
    ///
    /// This controls how long `receive_interrupt()` waits for device events.
    /// Shorter timeouts (100-500ms) allow responsive event loops; longer timeouts
    /// reduce polling overhead but block callers longer when no events are pending.
    pub fn set_event_timeout(&mut self, timeout: Duration) {
        self.event_timeout = timeout;
    }

    /// Convert a nusb TransferError to crate::Error.
    fn convert_transfer_error(err: TransferError) -> crate::Error {
        match err {
            // nusb returns Cancelled when transfer_blocking times out (it cancels
            // the transfer internally). Since we never explicitly cancel transfers,
            // Cancelled always means the timeout expired. Map to Timeout so that
            // Error::is_retryable() treats it correctly.
            TransferError::Cancelled => crate::Error::Timeout,
            TransferError::Disconnected => crate::Error::Disconnected,
            TransferError::Stall
            | TransferError::Fault
            | TransferError::InvalidArgument
            | TransferError::Unknown(_) => crate::Error::Io(std::io::Error::other(err.to_string())),
        }
    }
}

#[async_trait]
impl Transport for NusbTransport {
    async fn send_bulk(&self, data: &[u8]) -> Result<(), crate::Error> {
        let completion = {
            let mut ep = self.bulk_out.lock().expect("bulk_out mutex poisoned");
            let buf: Buffer = data.to_vec().into();
            ep.transfer_blocking(buf, self.timeout)
        };
        completion.status.map_err(Self::convert_transfer_error)?;
        Ok(())
    }

    async fn receive_bulk(&self, max_size: usize) -> Result<Vec<u8>, crate::Error> {
        let completion = {
            let mut ep = self.bulk_in.lock().expect("bulk_in mutex poisoned");

            // Align max_size up to max_packet_size for nusb 0.2's requirement
            // that IN transfer sizes are multiples of max_packet_size.
            let max_packet_size = ep.max_packet_size();
            let aligned_size = align_to_packet_size(max_size, max_packet_size);

            ep.transfer_blocking(Buffer::new(aligned_size), self.timeout)
        };
        completion.status.map_err(Self::convert_transfer_error)?;
        Ok(completion.buffer[..completion.actual_len].to_vec())
    }

    async fn receive_interrupt(&self) -> Result<Vec<u8>, crate::Error> {
        let completion = {
            let mut ep = self
                .interrupt_in
                .lock()
                .expect("interrupt_in mutex poisoned");

            // Align to max_packet_size
            let max_packet_size = ep.max_packet_size();
            let aligned_size = align_to_packet_size(Self::INTERRUPT_BUFFER_SIZE, max_packet_size);

            ep.transfer_blocking(Buffer::new(aligned_size), self.event_timeout)
        };
        completion.status.map_err(Self::convert_transfer_error)?;
        Ok(completion.buffer[..completion.actual_len].to_vec())
    }
}

/// Round `size` up to the nearest multiple of `packet_size`.
///
/// nusb 0.2 requires that IN transfer buffer sizes are non-zero multiples of
/// the endpoint's maximum packet size.
fn align_to_packet_size(size: usize, packet_size: usize) -> usize {
    if packet_size == 0 {
        return size.max(1);
    }
    if size == 0 {
        return packet_size;
    }
    if size % packet_size == 0 {
        size
    } else {
        ((size / packet_size) + 1) * packet_size
    }
}

/// Derive a stable location identifier from USB topology (bus + port chain).
///
/// Uses FNV-1a to hash `bus_id` and `port_chain` into a deterministic `u64`.
/// The result is stable across calls for the same physical USB port, regardless
/// of which device is plugged in.
fn location_id_from_topology(dev: &nusb::DeviceInfo) -> u64 {
    // FNV-1a 64-bit constants
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;

    let mut hash = FNV_OFFSET;
    for byte in dev.bus_id().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    // Separator so bus_id "1" + port [2,3] differs from bus_id "12" + port [3]
    hash ^= 0xFF;
    hash = hash.wrapping_mul(FNV_PRIME);
    for byte in dev.port_chain() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
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
                "  {:04x}:{:04x} serial={:?} location={:08x}",
                dev.vendor_id, dev.product_id, dev.serial_number, dev.location_id,
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
        assert_eq!(
            transport.event_timeout(),
            NusbTransport::DEFAULT_EVENT_TIMEOUT
        );
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

        // Bulk timeout should be custom, event timeout should be default
        assert_eq!(transport.timeout(), custom_timeout);
        assert_eq!(
            transport.event_timeout(),
            NusbTransport::DEFAULT_EVENT_TIMEOUT
        );

        // Test setters
        let new_timeout = Duration::from_secs(10);
        transport.set_timeout(new_timeout);
        assert_eq!(transport.timeout(), new_timeout);

        let new_event_timeout = Duration::from_millis(500);
        transport.set_event_timeout(new_event_timeout);
        assert_eq!(transport.event_timeout(), new_event_timeout);
    }

    #[tokio::test]
    #[ignore] // Requires real MTP device
    async fn test_open_with_timeouts() {
        let devices = NusbTransport::list_mtp_devices().unwrap();
        assert!(!devices.is_empty(), "No MTP device found");

        let device = devices[0].open().unwrap();
        let bulk_timeout = Duration::from_secs(45);
        let event_timeout = Duration::from_millis(100);
        let transport = NusbTransport::open_with_timeouts(device, bulk_timeout, event_timeout)
            .await
            .unwrap();

        assert_eq!(transport.timeout(), bulk_timeout);
        assert_eq!(transport.event_timeout(), event_timeout);
    }

    #[test]
    fn test_align_to_packet_size() {
        // Zero size rounds up to packet_size
        assert_eq!(align_to_packet_size(0, 512), 512);
        // Size smaller than packet rounds up
        assert_eq!(align_to_packet_size(1, 512), 512);
        // Exact multiple stays the same
        assert_eq!(align_to_packet_size(512, 512), 512);
        assert_eq!(align_to_packet_size(1024, 512), 1024);
        // Non-multiple rounds up
        assert_eq!(align_to_packet_size(513, 512), 1024);
        assert_eq!(align_to_packet_size(100, 64), 128);
        // Zero packet_size edge case
        assert_eq!(align_to_packet_size(0, 0), 1);
        assert_eq!(align_to_packet_size(100, 0), 100);
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
