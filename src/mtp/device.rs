//! MtpDevice - the main entry point for MTP operations.

use crate::mtp::{DeviceEvent, Storage};
use crate::ptp::{DeviceInfo, ObjectHandle, PtpSession, StorageId};
use crate::transport::{NusbTransport, Transport};
use crate::Error;
use std::sync::Arc;
use std::time::Duration;

/// Internal shared state for MtpDevice.
pub(crate) struct MtpDeviceInner {
    pub(crate) session: Arc<PtpSession>,
    pub(crate) device_info: DeviceInfo,
}

impl MtpDeviceInner {
    /// Check if the device is an Android device.
    ///
    /// Detected by looking for "android.com" in the vendor extension descriptor.
    /// Android devices have known MTP quirks (e.g., ObjectHandle::ALL doesn't work
    /// for recursive listing).
    pub fn is_android(&self) -> bool {
        self.device_info
            .vendor_extension_desc
            .to_lowercase()
            .contains("android.com")
    }
}

/// An MTP device connection.
///
/// This is the main entry point for interacting with MTP devices.
/// Use `MtpDevice::open_first()` to connect to the first available device,
/// or `MtpDevice::builder()` for more control.
///
/// # Example
///
/// ```rust,ignore
/// use mtp_rs::mtp::MtpDevice;
///
/// # async fn example() -> Result<(), mtp_rs::Error> {
/// // Open the first MTP device
/// let device = MtpDevice::open_first().await?;
///
/// println!("Connected to: {} {}",
///          device.device_info().manufacturer,
///          device.device_info().model);
///
/// // Get storages
/// for storage in device.storages().await? {
///     println!("Storage: {} ({} free)",
///              storage.info().description,
///              storage.info().free_space_bytes);
/// }
/// # Ok(())
/// # }
/// ```
pub struct MtpDevice {
    inner: Arc<MtpDeviceInner>,
}

impl MtpDevice {
    /// Create a builder for configuring device options.
    pub fn builder() -> MtpDeviceBuilder {
        MtpDeviceBuilder::new()
    }

    /// Open the first available MTP device with default settings.
    pub async fn open_first() -> Result<Self, Error> {
        Self::builder().open_first().await
    }

    /// Open a device at a specific USB location (port) with default settings.
    ///
    /// Use `list_devices()` to get available location IDs.
    pub async fn open_by_location(location_id: u64) -> Result<Self, Error> {
        Self::builder().open_by_location(location_id).await
    }

    /// Open a device by its serial number with default settings.
    ///
    /// This identifies a specific physical device regardless of which USB port
    /// it's connected to.
    pub async fn open_by_serial(serial: &str) -> Result<Self, Error> {
        Self::builder().open_by_serial(serial).await
    }

    /// List all available MTP devices without opening them.
    pub fn list_devices() -> Result<Vec<MtpDeviceInfo>, Error> {
        let devices = NusbTransport::list_mtp_devices()?;
        Ok(devices
            .into_iter()
            .map(|d| MtpDeviceInfo {
                vendor_id: d.vendor_id,
                product_id: d.product_id,
                manufacturer: d.manufacturer,
                product: d.product,
                serial_number: d.serial_number,
                location_id: d.location_id,
            })
            .collect())
    }

    /// Get device information.
    pub fn device_info(&self) -> &DeviceInfo {
        &self.inner.device_info
    }

    /// Check if the device supports renaming objects.
    ///
    /// This checks for support of the SetObjectPropValue operation (0x9804),
    /// which is required to rename files and folders via the ObjectFileName property.
    ///
    /// # Returns
    ///
    /// Returns true if the device advertises SetObjectPropValue support.
    pub fn supports_rename(&self) -> bool {
        self.inner.device_info.supports_rename()
    }

    /// Get all storages on the device.
    pub async fn storages(&self) -> Result<Vec<Storage>, Error> {
        let ids = self.inner.session.get_storage_ids().await?;
        let mut storages = Vec::with_capacity(ids.len());
        for id in ids {
            let info = self.inner.session.get_storage_info(id).await?;
            storages.push(Storage::new(self.inner.clone(), id, info));
        }
        Ok(storages)
    }

    /// Get a specific storage by ID.
    pub async fn storage(&self, id: StorageId) -> Result<Storage, Error> {
        let info = self.inner.session.get_storage_info(id).await?;
        Ok(Storage::new(self.inner.clone(), id, info))
    }

    /// Get object handles in a storage.
    ///
    /// # Arguments
    ///
    /// * `storage_id` - Storage to search, or `StorageId::ALL` for all storages
    /// * `parent` - Parent folder handle, or `None` for root level only,
    ///   or `Some(ObjectHandle::ALL)` for recursive listing
    pub async fn get_object_handles(
        &self,
        storage_id: StorageId,
        parent: Option<ObjectHandle>,
    ) -> Result<Vec<ObjectHandle>, Error> {
        self.inner
            .session
            .get_object_handles(storage_id, None, parent)
            .await
    }

    /// Receive the next event from the device.
    ///
    /// This method waits for an event on the USB interrupt endpoint, returning when
    /// an event arrives or the event timeout expires (default: 200ms). The short
    /// default timeout allows responsive event loops without blocking other operations.
    ///
    /// Configure the timeout via [`MtpDeviceBuilder::event_timeout()`].
    ///
    /// # Returns
    ///
    /// - `Ok(event)` - An event was received from the device
    /// - `Err(Error::Timeout)` - No event within the timeout period (normal, keep polling)
    /// - `Err(Error::Disconnected)` - Device was disconnected
    /// - `Err(_)` - Other communication error
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Configure event timeout if needed (default is 200ms)
    /// let device = MtpDevice::builder()
    ///     .event_timeout(Duration::from_millis(100))
    ///     .open_first()
    ///     .await?;
    ///
    /// loop {
    ///     match device.next_event().await {
    ///         Ok(event) => {
    ///             match event {
    ///                 DeviceEvent::ObjectAdded { handle } => {
    ///                     println!("New object: {:?}", handle);
    ///                 }
    ///                 DeviceEvent::StoreRemoved { storage_id } => {
    ///                     println!("Storage removed: {:?}", storage_id);
    ///                 }
    ///                 _ => {}
    ///             }
    ///         }
    ///         Err(Error::Disconnected) => break,
    ///         Err(Error::Timeout) => continue,  // No event, keep polling
    ///         Err(e) => {
    ///             eprintln!("Error: {}", e);
    ///             break;
    ///         }
    ///     }
    /// }
    /// ```
    pub async fn next_event(&self) -> Result<DeviceEvent, Error> {
        match self.inner.session.poll_event().await? {
            Some(container) => Ok(DeviceEvent::from_container(&container)),
            None => Err(Error::Timeout),
        }
    }

    /// Close the connection (also happens on drop).
    pub async fn close(self) -> Result<(), Error> {
        // Try to close gracefully, but Arc might have multiple references
        if let Ok(inner) = Arc::try_unwrap(self.inner) {
            if let Ok(session) = Arc::try_unwrap(inner.session) {
                session.close().await?;
            }
        }
        Ok(())
    }
}

/// Information about an MTP device (without opening it).
///
/// This struct provides device identification at multiple levels:
///
/// - **Device identity** (`vendor_id`, `product_id`, `serial_number`): Identifies
///   a specific physical device. Use this to recognize "John's phone" regardless
///   of which USB port it's plugged into.
///
/// - **Port identity** (`location_id`): Identifies the physical USB port/location.
///   Use this when you care about "the device on port 3" rather than which
///   specific device it is. Stable across reconnections to the same port.
///
/// - **Display info** (`manufacturer`, `product`): Human-readable strings for
///   showing device info to users.
///
/// # Example
///
/// ```rust,ignore
/// let devices = MtpDevice::list_devices()?;
/// for dev in &devices {
///     println!("{} {} (serial: {:?})",
///              dev.manufacturer.as_deref().unwrap_or("Unknown"),
///              dev.product.as_deref().unwrap_or("Unknown"),
///              dev.serial_number);
/// }
///
/// // Save location_id to remember "the device on this port"
/// // Save serial_number to remember "this specific phone"
/// ```
#[derive(Debug, Clone)]
pub struct MtpDeviceInfo {
    /// USB vendor ID (assigned by USB-IF to each company).
    ///
    /// Examples: Google = `0x18d1`, Samsung = `0x04e8`, Apple = `0x05ac`
    pub vendor_id: u16,

    /// USB product ID (assigned by vendor to each product model).
    ///
    /// Note: The same device may report different product IDs depending on
    /// its USB mode (MTP, ADB, charging-only, etc.).
    pub product_id: u16,

    /// Manufacturer name from USB descriptor.
    ///
    /// Examples: `"Google"`, `"Samsung"`, `"Apple Inc."`
    ///
    /// `None` if the device doesn't report a manufacturer string.
    pub manufacturer: Option<String>,

    /// Product name from USB descriptor.
    ///
    /// Examples: `"Pixel 9 Pro XL"`, `"Galaxy S24"`
    ///
    /// `None` if the device doesn't report a product string.
    pub product: Option<String>,

    /// Serial number uniquely identifying this specific device.
    ///
    /// Combined with `vendor_id` and `product_id`, this globally identifies
    /// a single physical device. Survives reconnection to different ports.
    ///
    /// `None` if the device doesn't report a serial number.
    pub serial_number: Option<String>,

    /// Physical USB location identifier.
    ///
    /// Identifies the USB port/path where the device is connected. Stable
    /// across reconnections to the same physical port, but changes if the
    /// device is moved to a different port.
    ///
    /// Platform details:
    /// - **macOS**: IOKit `locationID` encoding the port path
    /// - **Linux**: Derived from sysfs bus/port path
    /// - **Windows**: `LocationInformation` property
    pub location_id: u64,
}

impl MtpDeviceInfo {
    /// Format the device info for display.
    pub fn display(&self) -> String {
        let manufacturer = self.manufacturer.as_deref().unwrap_or("Unknown");
        let product = self.product.as_deref().unwrap_or("Unknown");
        match &self.serial_number {
            Some(serial) => format!(
                "{} {} (serial: {}, location: {:08x})",
                manufacturer, product, serial, self.location_id
            ),
            None => format!(
                "{} {} (location: {:08x})",
                manufacturer, product, self.location_id
            ),
        }
    }
}

/// Builder for MtpDevice configuration.
pub struct MtpDeviceBuilder {
    timeout: Duration,
    event_timeout: Duration,
}

impl MtpDeviceBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            timeout: NusbTransport::DEFAULT_TIMEOUT,
            event_timeout: NusbTransport::DEFAULT_EVENT_TIMEOUT,
        }
    }

    /// Set bulk transfer timeout (default: 30 seconds).
    ///
    /// This timeout applies to file transfers and command responses.
    /// Use longer timeouts for large file operations.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set event polling timeout (default: 200ms).
    ///
    /// This timeout controls how long `next_event()` waits for device events.
    /// Shorter timeouts (100-500ms) enable responsive event loops without
    /// blocking other operations. Longer timeouts reduce polling overhead.
    pub fn event_timeout(mut self, timeout: Duration) -> Self {
        self.event_timeout = timeout;
        self
    }

    /// Open the first available device.
    pub async fn open_first(self) -> Result<MtpDevice, Error> {
        let devices = NusbTransport::list_mtp_devices()?;
        let device_info = devices.into_iter().next().ok_or(Error::NoDevice)?;
        let device = device_info.open().map_err(Error::Usb)?;
        self.open_device(device).await
    }

    /// Open a device at a specific USB location (port).
    ///
    /// Use `MtpDevice::list_devices()` to get available location IDs.
    pub async fn open_by_location(self, location_id: u64) -> Result<MtpDevice, Error> {
        let devices = NusbTransport::list_mtp_devices()?;
        let device_info = devices
            .into_iter()
            .find(|d| d.location_id == location_id)
            .ok_or(Error::NoDevice)?;
        let device = device_info.open().map_err(Error::Usb)?;
        self.open_device(device).await
    }

    /// Open a device by its serial number.
    ///
    /// This identifies a specific physical device regardless of which USB port
    /// it's connected to.
    pub async fn open_by_serial(self, serial: &str) -> Result<MtpDevice, Error> {
        let devices = NusbTransport::list_mtp_devices()?;
        let device_info = devices
            .into_iter()
            .find(|d| d.serial_number.as_deref() == Some(serial))
            .ok_or(Error::NoDevice)?;
        let device = device_info.open().map_err(Error::Usb)?;
        self.open_device(device).await
    }

    /// Internal: open an already-discovered device.
    async fn open_device(self, device: nusb::Device) -> Result<MtpDevice, Error> {
        // Open transport
        let transport =
            NusbTransport::open_with_timeouts(device, self.timeout, self.event_timeout).await?;
        let transport: Arc<dyn Transport> = Arc::new(transport);

        // Open session (use session ID 1)
        let session = Arc::new(PtpSession::open(transport.clone(), 1).await?);

        // Get device info
        let device_info = session.get_device_info().await?;

        let inner = Arc::new(MtpDeviceInner {
            session,
            device_info,
        });

        Ok(MtpDevice { inner })
    }
}

impl Default for MtpDeviceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_devices() {
        // This test doesn't need mock - it just tests the function exists
        // Real devices test should be #[ignore]
        let result = MtpDevice::list_devices();
        // Will be empty or have devices depending on what's connected
        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_default() {
        let builder = MtpDeviceBuilder::new();
        assert_eq!(builder.timeout, NusbTransport::DEFAULT_TIMEOUT);
        assert_eq!(builder.event_timeout, NusbTransport::DEFAULT_EVENT_TIMEOUT);
    }

    #[test]
    fn test_builder_timeout() {
        let custom_timeout = Duration::from_secs(60);
        let builder = MtpDeviceBuilder::new().timeout(custom_timeout);
        assert_eq!(builder.timeout, custom_timeout);
        assert_eq!(builder.event_timeout, NusbTransport::DEFAULT_EVENT_TIMEOUT);
    }

    #[test]
    fn test_builder_event_timeout() {
        let custom_event_timeout = Duration::from_millis(500);
        let builder = MtpDeviceBuilder::new().event_timeout(custom_event_timeout);
        assert_eq!(builder.timeout, NusbTransport::DEFAULT_TIMEOUT);
        assert_eq!(builder.event_timeout, custom_event_timeout);
    }

    #[test]
    fn test_builder_both_timeouts() {
        let custom_timeout = Duration::from_secs(45);
        let custom_event_timeout = Duration::from_millis(100);
        let builder = MtpDeviceBuilder::new()
            .timeout(custom_timeout)
            .event_timeout(custom_event_timeout);
        assert_eq!(builder.timeout, custom_timeout);
        assert_eq!(builder.event_timeout, custom_event_timeout);
    }

    #[test]
    fn test_mtp_device_info_display() {
        let info = MtpDeviceInfo {
            vendor_id: 0x04e8,
            product_id: 0x6860,
            manufacturer: Some("Samsung".to_string()),
            product: Some("Galaxy S24".to_string()),
            serial_number: Some("ABC123".to_string()),
            location_id: 0x00200000,
        };
        let display = info.display();
        assert!(display.contains("Samsung"));
        assert!(display.contains("Galaxy S24"));
        assert!(display.contains("ABC123"));
        assert!(display.contains("00200000"));
    }

    #[test]
    fn test_mtp_device_info_display_no_serial() {
        let info = MtpDeviceInfo {
            vendor_id: 0x04e8,
            product_id: 0x6860,
            manufacturer: Some("Samsung".to_string()),
            product: Some("Galaxy S24".to_string()),
            serial_number: None,
            location_id: 0x00200000,
        };
        let display = info.display();
        assert!(display.contains("Samsung"));
        assert!(display.contains("Galaxy S24"));
        assert!(display.contains("00200000"));
        assert!(!display.contains("serial:"));
    }

    #[test]
    fn test_mtp_device_info_display_unknown_manufacturer() {
        let info = MtpDeviceInfo {
            vendor_id: 0x04e8,
            product_id: 0x6860,
            manufacturer: None,
            product: None,
            serial_number: None,
            location_id: 0x00200000,
        };
        let display = info.display();
        assert!(display.contains("Unknown"));
    }

    #[tokio::test]
    #[ignore] // Requires real MTP device
    async fn test_open_first_device() {
        let device = MtpDevice::open_first().await.unwrap();
        println!("Connected to: {}", device.device_info().model);

        let storages = device.storages().await.unwrap();
        for storage in &storages {
            println!("Storage: {}", storage.info().description);
        }

        device.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore] // Requires real MTP device
    async fn test_builder_with_custom_timeout() {
        let device = MtpDevice::builder()
            .timeout(Duration::from_secs(60))
            .open_first()
            .await
            .unwrap();

        let info = device.device_info();
        println!("Model: {}", info.model);
        println!("Manufacturer: {}", info.manufacturer);

        device.close().await.unwrap();
    }
}
