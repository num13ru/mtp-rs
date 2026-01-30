//! MtpDevice - the main entry point for MTP operations.

use crate::mtp::{DeviceEvent, Storage};
use crate::ptp::{DeviceInfo, ObjectHandle, PtpSession, StorageId};
use crate::transport::{NusbTransport, Transport};
use crate::Error;
use std::sync::Arc;
use std::time::Duration;

/// Internal shared state for MtpDevice.
pub(crate) struct MtpDeviceInner {
    pub(crate) session: PtpSession,
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

    /// Open a specific device by USB bus/address with default settings.
    pub async fn open(bus: u8, address: u8) -> Result<Self, Error> {
        Self::builder().open(bus, address).await
    }

    /// List all available MTP devices without opening them.
    pub fn list_devices() -> Result<Vec<MtpDeviceInfo>, Error> {
        let devices = NusbTransport::list_mtp_devices()?;
        Ok(devices
            .into_iter()
            .map(|d| MtpDeviceInfo {
                bus: d.bus_number(),
                address: d.device_address(),
                vendor_id: d.vendor_id(),
                product_id: d.product_id(),
            })
            .collect())
    }

    /// Get device information.
    pub fn device_info(&self) -> &DeviceInfo {
        &self.inner.device_info
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
    /// This method waits until an event is received from the USB interrupt endpoint.
    /// Events include object added/removed, storage changes, etc.
    ///
    /// # Example
    ///
    /// ```rust,ignore
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
    ///         Err(Error::Timeout) => continue,  // No event, keep waiting
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
            inner.session.close().await?;
        }
        Ok(())
    }
}

/// Information about an MTP device (without opening it).
#[derive(Debug, Clone)]
pub struct MtpDeviceInfo {
    /// USB bus number
    pub bus: u8,
    /// USB device address
    pub address: u8,
    /// USB vendor ID
    pub vendor_id: u16,
    /// USB product ID
    pub product_id: u16,
}

impl MtpDeviceInfo {
    /// Format the device info for display.
    pub fn display(&self) -> String {
        format!(
            "{:04x}:{:04x} at {}:{}",
            self.vendor_id, self.product_id, self.bus, self.address
        )
    }
}

/// Builder for MtpDevice configuration.
pub struct MtpDeviceBuilder {
    timeout: Duration,
}

impl MtpDeviceBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            timeout: NusbTransport::DEFAULT_TIMEOUT,
        }
    }

    /// Set operation timeout (default: 30 seconds).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Open the first available device.
    pub async fn open_first(self) -> Result<MtpDevice, Error> {
        let devices = NusbTransport::list_mtp_devices()?;
        let device_info = devices.into_iter().next().ok_or(Error::NoDevice)?;
        let device = device_info.open().map_err(Error::Usb)?;
        self.open_device(device).await
    }

    /// Open a specific device by bus/address.
    pub async fn open(self, bus: u8, address: u8) -> Result<MtpDevice, Error> {
        let devices = NusbTransport::list_mtp_devices()?;
        let device_info = devices
            .into_iter()
            .find(|d| d.bus_number() == bus && d.device_address() == address)
            .ok_or(Error::NoDevice)?;
        let device = device_info.open().map_err(Error::Usb)?;
        self.open_device(device).await
    }

    /// Internal: open an already-discovered device.
    async fn open_device(self, device: nusb::Device) -> Result<MtpDevice, Error> {
        // Open transport
        let transport = NusbTransport::open_with_timeout(device, self.timeout).await?;
        let transport: Arc<dyn Transport> = Arc::new(transport);

        // Open session (use session ID 1)
        let session = PtpSession::open(transport.clone(), 1).await?;

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
    }

    #[test]
    fn test_builder_timeout() {
        let custom_timeout = Duration::from_secs(60);
        let builder = MtpDeviceBuilder::new().timeout(custom_timeout);
        assert_eq!(builder.timeout, custom_timeout);
    }

    #[test]
    fn test_mtp_device_info_display() {
        let info = MtpDeviceInfo {
            bus: 1,
            address: 5,
            vendor_id: 0x04e8,
            product_id: 0x6860,
        };
        let display = info.display();
        assert!(display.contains("04e8:6860"));
        assert!(display.contains("1:5"));
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
