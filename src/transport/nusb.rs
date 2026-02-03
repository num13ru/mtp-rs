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

/// USB device information with platform-specific location ID.
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
    /// Physical USB location identifier (stable per port)
    pub location_id: u64,
    /// Reference to the underlying nusb device info for opening
    nusb_info: nusb::DeviceInfo,
}

impl UsbDeviceInfo {
    /// Open the USB device.
    pub fn open(&self) -> Result<nusb::Device, std::io::Error> {
        self.nusb_info.open()
    }
}

/// USB transport implementation using nusb.
pub struct NusbTransport {
    interface: nusb::Interface,
    bulk_in: u8,
    bulk_out: u8,
    interrupt_in: u8,
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
        // Get location IDs for all USB devices (platform-specific)
        let location_map = get_usb_location_ids();

        let devices = nusb::list_devices()
            .map_err(crate::Error::Usb)?
            .filter(Self::is_mtp_device)
            .map(|dev| {
                let location_id = find_location_id(&dev, &location_map);
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
        // We need to open the device and inspect the interfaces.
        if dev.class() == 0 {
            if let Ok(device) = dev.open() {
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
            event_timeout,
        })
    }

    /// Get the bulk transfer timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Set the bulk transfer timeout.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Get the event polling timeout.
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
            Box::pin(futures_timer::Delay::new(self.event_timeout)),
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

// ============================================================================
// Platform-specific location ID retrieval
// ============================================================================

/// Map of (vendor_id, product_id, serial) -> location_id
type LocationMap = std::collections::HashMap<(u16, u16, Option<String>), u64>;

/// Find the location_id for a device using the pre-built map.
fn find_location_id(dev: &nusb::DeviceInfo, map: &LocationMap) -> u64 {
    let key = (
        dev.vendor_id(),
        dev.product_id(),
        dev.serial_number().map(String::from),
    );
    if let Some(&loc) = map.get(&key) {
        return loc;
    }

    // Fallback: try without serial (some devices don't report serial before open)
    let key_no_serial = (dev.vendor_id(), dev.product_id(), None);
    if let Some(&loc) = map.get(&key_no_serial) {
        return loc;
    }

    // Last resort fallback: combine bus and address (works on Linux)
    // This is not unique on macOS but better than nothing
    ((dev.bus_number() as u64) << 32) | (dev.device_address() as u64)
}

// macOS implementation using IOKit
#[cfg(target_os = "macos")]
fn get_usb_location_ids() -> LocationMap {
    use io_kit_sys::types::io_iterator_t;
    use io_kit_sys::*;

    let mut map = LocationMap::new();

    unsafe {
        // Create matching dictionary for USB devices
        let matching = IOServiceMatching(usb::lib::kIOUSBDeviceClassName);
        if matching.is_null() {
            return map;
        }

        // Get iterator for matching services
        let mut iterator: io_iterator_t = 0;
        #[allow(deprecated)]
        let result = IOServiceGetMatchingServices(kIOMasterPortDefault, matching, &mut iterator);
        if result != ret::kIOReturnSuccess {
            return map;
        }

        // Iterate through USB devices
        loop {
            let service = IOIteratorNext(iterator);
            if service == 0 {
                break;
            }

            // Get properties we need
            let vendor_id = get_iokit_property_number(service, "idVendor").unwrap_or(0) as u16;
            let product_id = get_iokit_property_number(service, "idProduct").unwrap_or(0) as u16;
            let location_id = get_iokit_property_number(service, "locationID").unwrap_or(0) as u64;
            let serial = get_iokit_property_string(service, "USB Serial Number");

            if vendor_id != 0 && location_id != 0 {
                map.insert((vendor_id, product_id, serial), location_id);
            }

            IOObjectRelease(service);
        }

        IOObjectRelease(iterator);
    }

    map
}

#[cfg(target_os = "macos")]
unsafe fn get_iokit_property_number(
    service: io_kit_sys::types::io_service_t,
    key: &str,
) -> Option<i64> {
    use core_foundation::base::{kCFAllocatorDefault, TCFType};
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use io_kit_sys::*;

    let cf_key = CFString::new(key);
    let cf_value = IORegistryEntryCreateCFProperty(
        service,
        cf_key.as_concrete_TypeRef() as _,
        kCFAllocatorDefault,
        0,
    );
    if cf_value.is_null() {
        return None;
    }

    let number = CFNumber::wrap_under_create_rule(cf_value as _);
    number.to_i64()
}

#[cfg(target_os = "macos")]
unsafe fn get_iokit_property_string(
    service: io_kit_sys::types::io_service_t,
    key: &str,
) -> Option<String> {
    use core_foundation::base::{kCFAllocatorDefault, TCFType};
    use core_foundation::string::CFString;
    use io_kit_sys::*;

    let cf_key = CFString::new(key);
    let cf_value = IORegistryEntryCreateCFProperty(
        service,
        cf_key.as_concrete_TypeRef() as _,
        kCFAllocatorDefault,
        0,
    );
    if cf_value.is_null() {
        return None;
    }

    let cf_string = CFString::wrap_under_create_rule(cf_value as _);
    Some(cf_string.to_string())
}

// Linux: bus:address is reliable there, so we use that as location_id
#[cfg(target_os = "linux")]
fn get_usb_location_ids() -> LocationMap {
    // On Linux, bus:address is unique and stable per port
    // We'll build the location_id from bus/address in find_location_id fallback
    LocationMap::new()
}

// Windows and other platforms: fallback to bus:address
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn get_usb_location_ids() -> LocationMap {
    // TODO: Implement Windows LocationInformation retrieval
    LocationMap::new()
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
