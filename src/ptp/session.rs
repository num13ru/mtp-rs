//! PTP session management.
//!
//! This module provides session-level operations for MTP/PTP communication.
//! A session maintains the connection state and serializes concurrent operations.

use crate::ptp::{
    container_type, pack_string, pack_u16, pack_u32, unpack_u32, unpack_u32_array,
    CommandContainer, ContainerType, DataContainer, DeviceInfo, DevicePropDesc, DevicePropertyCode,
    EventContainer, ObjectFormatCode, ObjectHandle, ObjectInfo, ObjectPropertyCode, OperationCode,
    PropertyDataType, PropertyValue, ResponseCode, ResponseContainer, SessionId, StorageId,
    StorageInfo, TransactionId,
};
use crate::transport::Transport;
use crate::Error;
use bytes::Bytes;
use futures::lock::{Mutex, OwnedMutexGuard};
use futures::Stream;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Container header size in bytes.
const HEADER_SIZE: usize = 12;

/// A PTP session with a device.
///
/// PtpSession manages the lifecycle of a PTP/MTP session, including:
/// - Opening and closing sessions
/// - Transaction ID management
/// - Serializing concurrent operations (MTP only allows one operation at a time)
/// - Executing operations and receiving responses
///
/// # Example
///
/// ```rust,ignore
/// use mtp_rs::ptp::PtpSession;
///
/// // Open a session with session ID 1
/// let session = PtpSession::open(transport, 1).await?;
///
/// // Get device info
/// let device_info = session.get_device_info().await?;
///
/// // Get storage IDs
/// let storage_ids = session.get_storage_ids().await?;
///
/// // Close the session when done
/// session.close().await?;
/// ```
pub struct PtpSession {
    /// The transport layer for USB communication.
    transport: Arc<dyn Transport>,
    /// The session ID assigned to this session.
    session_id: SessionId,
    /// Atomic counter for generating transaction IDs.
    transaction_id: AtomicU32,
    /// Mutex to serialize operations (MTP only allows one operation at a time).
    /// Wrapped in Arc so it can be shared with ReceiveStream.
    operation_lock: Arc<Mutex<()>>,
}

impl PtpSession {
    /// Create a new session (internal, use open() to start session).
    fn new(transport: Arc<dyn Transport>, session_id: SessionId) -> Self {
        Self {
            transport,
            session_id,
            transaction_id: AtomicU32::new(TransactionId::FIRST.0),
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Open a new session with the device.
    ///
    /// This sends an OpenSession command to the device and establishes a session
    /// with the given session ID.
    ///
    /// # Arguments
    ///
    /// * `transport` - The transport layer for USB communication
    /// * `session_id` - The session ID to use (typically 1)
    ///
    /// # Errors
    ///
    /// Returns an error if the device rejects the session or communication fails.
    pub async fn open(transport: Arc<dyn Transport>, session_id: u32) -> Result<Self, Error> {
        let session = Self::new(transport, SessionId(session_id));

        // Send OpenSession command
        let response = session
            .execute(OperationCode::OpenSession, &[session_id])
            .await?;

        if response.code == ResponseCode::Ok {
            return Ok(session);
        }

        if response.code == ResponseCode::SessionAlreadyOpen {
            // Session already exists with potentially mismatched transaction ID.
            // Close the existing session (ignore errors) and open a fresh one.
            let _ = session.execute(OperationCode::CloseSession, &[]).await;

            // Create a new session instance with reset transaction ID counter
            let fresh_session = Self::new(Arc::clone(&session.transport), SessionId(session_id));

            let retry_response = fresh_session
                .execute(OperationCode::OpenSession, &[session_id])
                .await?;

            if retry_response.code != ResponseCode::Ok {
                return Err(Error::Protocol {
                    code: retry_response.code,
                    operation: OperationCode::OpenSession,
                });
            }

            return Ok(fresh_session);
        }

        Err(Error::Protocol {
            code: response.code,
            operation: OperationCode::OpenSession,
        })
    }

    /// Get the session ID.
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Close the session.
    ///
    /// This sends a CloseSession command to the device. Errors during close
    /// are ignored since the session is being terminated anyway.
    pub async fn close(self) -> Result<(), Error> {
        let _ = self.execute(OperationCode::CloseSession, &[]).await;
        Ok(())
    }

    /// Get the next transaction ID.
    ///
    /// Transaction IDs start at 1 and wrap correctly, skipping 0 and 0xFFFFFFFF.
    fn next_transaction_id(&self) -> u32 {
        loop {
            let current = self.transaction_id.load(Ordering::SeqCst);
            let next = TransactionId(current).next().0;
            if self
                .transaction_id
                .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return current;
            }
        }
    }

    // =========================================================================
    // Core operation execution
    // =========================================================================

    /// Execute an operation without data phase.
    async fn execute(
        &self,
        operation: OperationCode,
        params: &[u32],
    ) -> Result<ResponseContainer, Error> {
        let _guard = self.operation_lock.lock().await;

        let tx_id = self.next_transaction_id();

        // Build and send command
        let cmd = CommandContainer {
            code: operation,
            transaction_id: tx_id,
            params: params.to_vec(),
        };
        self.transport.send_bulk(&cmd.to_bytes()).await?;

        // Receive response
        let response_bytes = self.transport.receive_bulk(512).await?;
        let response = ResponseContainer::from_bytes(&response_bytes)?;

        // Verify transaction ID matches
        if response.transaction_id != tx_id {
            return Err(Error::invalid_data(format!(
                "Transaction ID mismatch: expected {}, got {}",
                tx_id, response.transaction_id
            )));
        }

        Ok(response)
    }

    /// Execute operation with data receive phase.
    async fn execute_with_receive(
        &self,
        operation: OperationCode,
        params: &[u32],
    ) -> Result<(ResponseContainer, Vec<u8>), Error> {
        let _guard = self.operation_lock.lock().await;

        let tx_id = self.next_transaction_id();

        // Send command
        let cmd = CommandContainer {
            code: operation,
            transaction_id: tx_id,
            params: params.to_vec(),
        };
        self.transport.send_bulk(&cmd.to_bytes()).await?;

        // Receive data container(s)
        // MTP sends data in one or more containers, then response.
        // A single data container may span multiple USB transfers if larger than 64KB.
        let mut data = Vec::new();

        loop {
            let mut bytes = self.transport.receive_bulk(64 * 1024).await?;
            if bytes.is_empty() {
                return Err(Error::invalid_data("Empty response"));
            }

            let ct = container_type(&bytes)?;
            match ct {
                ContainerType::Data => {
                    // Check if we need to receive more data for this container.
                    // The length field in the header tells us the total container size.
                    if bytes.len() >= 4 {
                        let total_length = unpack_u32(&bytes[0..4])? as usize;
                        // Keep receiving until we have the complete container
                        while bytes.len() < total_length {
                            let more = self.transport.receive_bulk(64 * 1024).await?;
                            if more.is_empty() {
                                return Err(Error::invalid_data(
                                    "Incomplete data container: device stopped sending",
                                ));
                            }
                            bytes.extend_from_slice(&more);
                        }
                    }
                    let container = DataContainer::from_bytes(&bytes)?;
                    data.extend_from_slice(&container.payload);
                    // Continue to receive more containers or response
                }
                ContainerType::Response => {
                    let response = ResponseContainer::from_bytes(&bytes)?;
                    if response.transaction_id != tx_id {
                        return Err(Error::invalid_data(format!(
                            "Transaction ID mismatch: expected {}, got {}",
                            tx_id, response.transaction_id
                        )));
                    }
                    return Ok((response, data));
                }
                _ => {
                    return Err(Error::invalid_data(format!(
                        "Unexpected container type: {:?}",
                        ct
                    )));
                }
            }
        }
    }

    /// Execute operation with data send phase.
    async fn execute_with_send(
        &self,
        operation: OperationCode,
        params: &[u32],
        data: &[u8],
    ) -> Result<ResponseContainer, Error> {
        let _guard = self.operation_lock.lock().await;

        let tx_id = self.next_transaction_id();

        // Send command
        let cmd = CommandContainer {
            code: operation,
            transaction_id: tx_id,
            params: params.to_vec(),
        };
        self.transport.send_bulk(&cmd.to_bytes()).await?;

        // Send data
        let data_container = DataContainer {
            code: operation,
            transaction_id: tx_id,
            payload: data.to_vec(),
        };
        self.transport.send_bulk(&data_container.to_bytes()).await?;

        // Receive response
        let response_bytes = self.transport.receive_bulk(512).await?;
        let response = ResponseContainer::from_bytes(&response_bytes)?;

        if response.transaction_id != tx_id {
            return Err(Error::invalid_data(format!(
                "Transaction ID mismatch: expected {}, got {}",
                tx_id, response.transaction_id
            )));
        }

        Ok(response)
    }

    // =========================================================================
    // High-level operations
    // =========================================================================

    /// Get device info.
    ///
    /// Returns information about the device including its capabilities,
    /// manufacturer, model, and supported operations.
    pub async fn get_device_info(&self) -> Result<DeviceInfo, Error> {
        let (response, data) = self
            .execute_with_receive(OperationCode::GetDeviceInfo, &[])
            .await?;
        Self::check_response(&response, OperationCode::GetDeviceInfo)?;
        DeviceInfo::from_bytes(&data)
    }

    /// Get storage IDs.
    ///
    /// Returns a list of storage IDs available on the device.
    pub async fn get_storage_ids(&self) -> Result<Vec<StorageId>, Error> {
        let (response, data) = self
            .execute_with_receive(OperationCode::GetStorageIds, &[])
            .await?;
        Self::check_response(&response, OperationCode::GetStorageIds)?;
        let (ids, _) = unpack_u32_array(&data)?;
        Ok(ids.into_iter().map(StorageId).collect())
    }

    /// Get storage info.
    ///
    /// Returns information about a specific storage, including capacity,
    /// free space, and filesystem type.
    pub async fn get_storage_info(&self, storage_id: StorageId) -> Result<StorageInfo, Error> {
        let (response, data) = self
            .execute_with_receive(OperationCode::GetStorageInfo, &[storage_id.0])
            .await?;
        Self::check_response(&response, OperationCode::GetStorageInfo)?;
        StorageInfo::from_bytes(&data)
    }

    /// Get object handles.
    ///
    /// Returns a list of object handles matching the specified criteria.
    ///
    /// # Arguments
    ///
    /// * `storage_id` - Storage to search, or `StorageId::ALL` for all storages
    /// * `format` - Filter by format, or `None` for all formats
    /// * `parent` - Parent folder handle, or `None` for root level only,
    ///   or `Some(ObjectHandle::ALL)` for recursive listing
    pub async fn get_object_handles(
        &self,
        storage_id: StorageId,
        format: Option<ObjectFormatCode>,
        parent: Option<ObjectHandle>,
    ) -> Result<Vec<ObjectHandle>, Error> {
        let format_code = format.map(|f| f.to_code() as u32).unwrap_or(0);
        let parent_handle = parent.map(|p| p.0).unwrap_or(0); // 0 = root only

        let (response, data) = self
            .execute_with_receive(
                OperationCode::GetObjectHandles,
                &[storage_id.0, format_code, parent_handle],
            )
            .await?;
        Self::check_response(&response, OperationCode::GetObjectHandles)?;
        let (handles, _) = unpack_u32_array(&data)?;
        Ok(handles.into_iter().map(ObjectHandle).collect())
    }

    /// Get object info.
    ///
    /// Returns metadata about an object, including filename, size, and timestamps.
    pub async fn get_object_info(&self, handle: ObjectHandle) -> Result<ObjectInfo, Error> {
        let (response, data) = self
            .execute_with_receive(OperationCode::GetObjectInfo, &[handle.0])
            .await?;
        Self::check_response(&response, OperationCode::GetObjectInfo)?;
        ObjectInfo::from_bytes(&data)
    }

    /// Get object (download).
    ///
    /// Downloads the complete data of an object.
    pub async fn get_object(&self, handle: ObjectHandle) -> Result<Vec<u8>, Error> {
        let (response, data) = self
            .execute_with_receive(OperationCode::GetObject, &[handle.0])
            .await?;
        Self::check_response(&response, OperationCode::GetObject)?;
        Ok(data)
    }

    /// Get partial object.
    ///
    /// Downloads a portion of an object's data.
    ///
    /// # Arguments
    ///
    /// * `handle` - The object handle
    /// * `offset` - Byte offset to start from (truncated to u32 in standard MTP)
    /// * `max_bytes` - Maximum number of bytes to retrieve
    pub async fn get_partial_object(
        &self,
        handle: ObjectHandle,
        offset: u64,
        max_bytes: u32,
    ) -> Result<Vec<u8>, Error> {
        // GetPartialObject params: handle, offset (u32), max_bytes (u32)
        // Note: offset is truncated to u32 in standard MTP
        let (response, data) = self
            .execute_with_receive(
                OperationCode::GetPartialObject,
                &[handle.0, offset as u32, max_bytes],
            )
            .await?;
        Self::check_response(&response, OperationCode::GetPartialObject)?;
        Ok(data)
    }

    /// Get thumbnail.
    ///
    /// Downloads the thumbnail image for an object.
    pub async fn get_thumb(&self, handle: ObjectHandle) -> Result<Vec<u8>, Error> {
        let (response, data) = self
            .execute_with_receive(OperationCode::GetThumb, &[handle.0])
            .await?;
        Self::check_response(&response, OperationCode::GetThumb)?;
        Ok(data)
    }

    /// Send object info (prepare for upload).
    ///
    /// This must be called before `send_object()` to prepare the device for
    /// receiving a new object.
    ///
    /// # Returns
    ///
    /// Returns a tuple of (storage_id, parent_handle, new_object_handle) where:
    /// - `storage_id` - The storage where the object will be created
    /// - `parent_handle` - The parent folder handle
    /// - `new_object_handle` - The handle assigned to the new object
    pub async fn send_object_info(
        &self,
        storage_id: StorageId,
        parent: ObjectHandle,
        info: &ObjectInfo,
    ) -> Result<(StorageId, ObjectHandle, ObjectHandle), Error> {
        let data = info.to_bytes()?;
        let response = self
            .execute_with_send(
                OperationCode::SendObjectInfo,
                &[storage_id.0, parent.0],
                &data,
            )
            .await?;
        Self::check_response(&response, OperationCode::SendObjectInfo)?;

        // Response params: storage_id, parent_handle, object_handle
        if response.params.len() < 3 {
            return Err(Error::invalid_data(
                "SendObjectInfo response missing params",
            ));
        }
        Ok((
            StorageId(response.params[0]),
            ObjectHandle(response.params[1]),
            ObjectHandle(response.params[2]),
        ))
    }

    /// Send object data (must follow send_object_info).
    ///
    /// Uploads the actual data for an object. This must be called immediately
    /// after `send_object_info()`.
    pub async fn send_object(&self, data: &[u8]) -> Result<(), Error> {
        let response = self
            .execute_with_send(OperationCode::SendObject, &[], data)
            .await?;
        Self::check_response(&response, OperationCode::SendObject)?;
        Ok(())
    }

    /// Delete object.
    ///
    /// Deletes an object from the device.
    pub async fn delete_object(&self, handle: ObjectHandle) -> Result<(), Error> {
        // Param2 is format code, 0 means any format
        let response = self
            .execute(OperationCode::DeleteObject, &[handle.0, 0])
            .await?;
        Self::check_response(&response, OperationCode::DeleteObject)?;
        Ok(())
    }

    /// Move object.
    ///
    /// Moves an object to a different location.
    pub async fn move_object(
        &self,
        handle: ObjectHandle,
        storage_id: StorageId,
        parent: ObjectHandle,
    ) -> Result<(), Error> {
        let response = self
            .execute(
                OperationCode::MoveObject,
                &[handle.0, storage_id.0, parent.0],
            )
            .await?;
        Self::check_response(&response, OperationCode::MoveObject)?;
        Ok(())
    }

    /// Copy object.
    ///
    /// Copies an object to a new location.
    ///
    /// # Returns
    ///
    /// Returns the handle of the newly created copy.
    pub async fn copy_object(
        &self,
        handle: ObjectHandle,
        storage_id: StorageId,
        parent: ObjectHandle,
    ) -> Result<ObjectHandle, Error> {
        let response = self
            .execute(
                OperationCode::CopyObject,
                &[handle.0, storage_id.0, parent.0],
            )
            .await?;
        Self::check_response(&response, OperationCode::CopyObject)?;

        if response.params.is_empty() {
            return Err(Error::invalid_data("CopyObject response missing handle"));
        }
        Ok(ObjectHandle(response.params[0]))
    }

    /// Get object property value.
    ///
    /// Retrieves the value of a specific property for an object.
    /// This is an MTP extension operation (0x9803).
    ///
    /// # Arguments
    ///
    /// * `handle` - The object handle
    /// * `property` - The property code to retrieve
    ///
    /// # Returns
    ///
    /// Returns the raw property value as bytes.
    pub async fn get_object_prop_value(
        &self,
        handle: ObjectHandle,
        property: ObjectPropertyCode,
    ) -> Result<Vec<u8>, Error> {
        let (response, data) = self
            .execute_with_receive(
                OperationCode::GetObjectPropValue,
                &[handle.0, property.to_code() as u32],
            )
            .await?;
        Self::check_response(&response, OperationCode::GetObjectPropValue)?;
        Ok(data)
    }

    /// Set object property value.
    ///
    /// Sets the value of a specific property for an object.
    /// This is an MTP extension operation (0x9804).
    ///
    /// # Arguments
    ///
    /// * `handle` - The object handle
    /// * `property` - The property code to set
    /// * `value` - The raw property value as bytes
    pub async fn set_object_prop_value(
        &self,
        handle: ObjectHandle,
        property: ObjectPropertyCode,
        value: &[u8],
    ) -> Result<(), Error> {
        let response = self
            .execute_with_send(
                OperationCode::SetObjectPropValue,
                &[handle.0, property.to_code() as u32],
                value,
            )
            .await?;
        Self::check_response(&response, OperationCode::SetObjectPropValue)?;
        Ok(())
    }

    /// Rename an object (file or folder).
    ///
    /// This is a convenience method that uses SetObjectPropValue to change
    /// the ObjectFileName property (0xDC07).
    ///
    /// # Arguments
    ///
    /// * `handle` - The object handle to rename
    /// * `new_name` - The new filename
    ///
    /// # Note
    ///
    /// Not all devices support renaming. Check `supports_rename()` on DeviceInfo first.
    pub async fn rename_object(&self, handle: ObjectHandle, new_name: &str) -> Result<(), Error> {
        let name_bytes = pack_string(new_name);
        self.set_object_prop_value(handle, ObjectPropertyCode::ObjectFileName, &name_bytes)
            .await
    }

    // =========================================================================
    // Device property operations
    // =========================================================================

    /// Get the descriptor for a device property.
    ///
    /// Returns detailed information about the property including its type,
    /// current value, default value, and allowed values/range.
    ///
    /// This is primarily used for digital cameras to query settings like
    /// ISO, aperture, shutter speed, etc. Most Android MTP devices do not
    /// support device properties.
    ///
    /// # Arguments
    ///
    /// * `property` - The device property code to query
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let desc = session.get_device_prop_desc(DevicePropertyCode::BatteryLevel).await?;
    /// println!("Battery level: {:?}", desc.current_value);
    /// ```
    pub async fn get_device_prop_desc(
        &self,
        property: DevicePropertyCode,
    ) -> Result<DevicePropDesc, Error> {
        let (response, data) = self
            .execute_with_receive(
                OperationCode::GetDevicePropDesc,
                &[property.to_code() as u32],
            )
            .await?;
        Self::check_response(&response, OperationCode::GetDevicePropDesc)?;
        DevicePropDesc::from_bytes(&data)
    }

    /// Get the current value of a device property.
    ///
    /// Returns the raw bytes of the property value. To interpret the value,
    /// you need to know the property's data type. Use `get_device_prop_desc()`
    /// to get the full descriptor including the data type.
    ///
    /// # Arguments
    ///
    /// * `property` - The device property code to query
    pub async fn get_device_prop_value(
        &self,
        property: DevicePropertyCode,
    ) -> Result<Vec<u8>, Error> {
        let (response, data) = self
            .execute_with_receive(
                OperationCode::GetDevicePropValue,
                &[property.to_code() as u32],
            )
            .await?;
        Self::check_response(&response, OperationCode::GetDevicePropValue)?;
        Ok(data)
    }

    /// Get a device property value as a typed PropertyValue.
    ///
    /// This is a convenience method that parses the raw bytes according to
    /// the specified data type.
    ///
    /// # Arguments
    ///
    /// * `property` - The device property code to query
    /// * `data_type` - The expected data type of the property
    pub async fn get_device_prop_value_typed(
        &self,
        property: DevicePropertyCode,
        data_type: PropertyDataType,
    ) -> Result<PropertyValue, Error> {
        let data = self.get_device_prop_value(property).await?;
        let (value, _) = PropertyValue::from_bytes(&data, data_type)?;
        Ok(value)
    }

    /// Set a device property value.
    ///
    /// The value should be the raw bytes of the new value. The value type
    /// must match the property's data type.
    ///
    /// # Arguments
    ///
    /// * `property` - The device property code to set
    /// * `value` - The raw bytes of the new value
    pub async fn set_device_prop_value(
        &self,
        property: DevicePropertyCode,
        value: &[u8],
    ) -> Result<(), Error> {
        let response = self
            .execute_with_send(
                OperationCode::SetDevicePropValue,
                &[property.to_code() as u32],
                value,
            )
            .await?;
        Self::check_response(&response, OperationCode::SetDevicePropValue)?;
        Ok(())
    }

    /// Set a device property value from a PropertyValue.
    ///
    /// This is a convenience method that serializes the PropertyValue to bytes.
    ///
    /// # Arguments
    ///
    /// * `property` - The device property code to set
    /// * `value` - The new value
    pub async fn set_device_prop_value_typed(
        &self,
        property: DevicePropertyCode,
        value: &PropertyValue,
    ) -> Result<(), Error> {
        let data = value.to_bytes();
        self.set_device_prop_value(property, &data).await
    }

    /// Reset a device property to its default value.
    ///
    /// # Arguments
    ///
    /// * `property` - The device property code to reset
    pub async fn reset_device_prop_value(&self, property: DevicePropertyCode) -> Result<(), Error> {
        let response = self
            .execute(
                OperationCode::ResetDevicePropValue,
                &[property.to_code() as u32],
            )
            .await?;
        Self::check_response(&response, OperationCode::ResetDevicePropValue)?;
        Ok(())
    }

    // =========================================================================
    // Capture operations
    // =========================================================================

    /// Initiate a capture operation.
    ///
    /// This triggers the camera to capture an image. The operation is asynchronous;
    /// use `poll_event()` to wait for `CaptureComplete` and `ObjectAdded` events.
    ///
    /// # Arguments
    ///
    /// * `storage_id` - Target storage (use `StorageId(0)` for camera default)
    /// * `format` - Object format for the capture (use `ObjectFormatCode::Undefined`
    ///   for camera default)
    ///
    /// # Events
    ///
    /// After calling this method, monitor for these events:
    /// - `EventCode::CaptureComplete` - Capture operation finished
    /// - `EventCode::ObjectAdded` - New object (image) was created on device
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Trigger capture
    /// session.initiate_capture(StorageId(0), ObjectFormatCode::Undefined).await?;
    ///
    /// // Wait for events
    /// loop {
    ///     match session.poll_event().await? {
    ///         Some(event) if event.code == EventCode::CaptureComplete => {
    ///             println!("Capture complete!");
    ///             break;
    ///         }
    ///         Some(event) if event.code == EventCode::ObjectAdded => {
    ///             println!("New object: {}", event.params[0]);
    ///         }
    ///         _ => continue,
    ///     }
    /// }
    /// ```
    pub async fn initiate_capture(
        &self,
        storage_id: StorageId,
        format: ObjectFormatCode,
    ) -> Result<(), Error> {
        // Per PTP spec, 0x00000000 means "any format" / "use device default".
        // ObjectFormatCode::Undefined (0x3000) is different and may not be accepted.
        let format_code = match format {
            ObjectFormatCode::Undefined => 0,
            other => other.to_code() as u32,
        };
        let response = self
            .execute(OperationCode::InitiateCapture, &[storage_id.0, format_code])
            .await?;
        Self::check_response(&response, OperationCode::InitiateCapture)?;
        Ok(())
    }

    // =========================================================================
    // Event handling
    // =========================================================================

    /// Poll for a single event from the interrupt endpoint.
    ///
    /// This method waits until an event is received from the USB interrupt endpoint.
    /// Events are asynchronous notifications from the device about changes such as
    /// objects being added/removed, storage changes, etc.
    ///
    /// Note: This method does not require the operation lock since events are
    /// received on the interrupt endpoint, which is independent of bulk transfers.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(container))` - An event was received
    /// - `Ok(None)` - Timeout occurred (no event available)
    /// - `Err(_)` - Communication error
    pub async fn poll_event(&self) -> Result<Option<EventContainer>, Error> {
        match self.transport.receive_interrupt().await {
            Ok(bytes) => {
                let container = EventContainer::from_bytes(&bytes)?;
                Ok(Some(container))
            }
            Err(Error::Timeout) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    /// Helper to check response is OK.
    fn check_response(response: &ResponseContainer, operation: OperationCode) -> Result<(), Error> {
        if response.code == ResponseCode::Ok {
            Ok(())
        } else {
            Err(Error::Protocol {
                code: response.code,
                operation,
            })
        }
    }

    // =========================================================================
    // Streaming operations
    // =========================================================================

    /// Execute operation with streaming data receive.
    ///
    /// Returns a Stream that yields data chunks as they arrive from USB.
    /// The stream yields `Bytes` chunks (typically up to 64KB each).
    ///
    /// # Important
    ///
    /// The caller must consume the entire stream before calling any other
    /// session methods. The MTP session is locked while the stream is active.
    ///
    /// # Arguments
    ///
    /// * `operation` - The operation code to execute
    /// * `params` - Operation parameters
    ///
    /// # Returns
    ///
    /// A `ReceiveStream` that yields `Result<Bytes, Error>` chunks.
    pub async fn execute_with_receive_stream(
        self: &Arc<Self>,
        operation: OperationCode,
        params: &[u32],
    ) -> Result<ReceiveStream, Error> {
        // Clone the Arc for the lock
        let lock = Arc::clone(&self.operation_lock);
        let guard = lock.lock_owned().await;

        let tx_id = self.next_transaction_id();

        // Send command
        let cmd = CommandContainer {
            code: operation,
            transaction_id: tx_id,
            params: params.to_vec(),
        };
        self.transport.send_bulk(&cmd.to_bytes()).await?;

        Ok(ReceiveStream {
            transport: Arc::clone(&self.transport),
            _guard: guard,
            transaction_id: tx_id,
            operation,
            buffer: Vec::new(),
            container_length: 0,
            payload_yielded: 0,
            header_parsed: false,
            done: false,
        })
    }

    /// Execute operation with streaming data send.
    ///
    /// Accepts a Stream of data chunks to send. The total_size must be
    /// known upfront (MTP protocol requirement).
    ///
    /// # Arguments
    ///
    /// * `operation` - The operation code
    /// * `params` - Operation parameters
    /// * `total_size` - Total bytes that will be sent (REQUIRED by MTP protocol)
    /// * `data` - Stream of data chunks to send
    ///
    /// # Important
    ///
    /// The `total_size` must match the actual total bytes in the stream.
    /// MTP requires knowing the size before transfer begins.
    pub async fn execute_with_send_stream<S>(
        &self,
        operation: OperationCode,
        params: &[u32],
        total_size: u64,
        mut data: S,
    ) -> Result<ResponseContainer, Error>
    where
        S: Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
    {
        use futures::StreamExt;

        let _guard = self.operation_lock.lock().await;
        let tx_id = self.next_transaction_id();

        // Send command
        let cmd = CommandContainer {
            code: operation,
            transaction_id: tx_id,
            params: params.to_vec(),
        };
        self.transport.send_bulk(&cmd.to_bytes()).await?;

        // Build complete data container (header + all payload)
        // MTP devices expect the entire data container in a single USB transfer
        let container_length = HEADER_SIZE as u64 + total_size;
        let mut buffer = Vec::with_capacity(container_length as usize);

        // Add header
        if container_length <= u32::MAX as u64 {
            buffer.extend_from_slice(&pack_u32(container_length as u32));
        } else {
            buffer.extend_from_slice(&pack_u32(0xFFFFFFFF));
        }
        buffer.extend_from_slice(&pack_u16(ContainerType::Data.to_code()));
        buffer.extend_from_slice(&pack_u16(operation.to_code()));
        buffer.extend_from_slice(&pack_u32(tx_id));

        // Collect all chunks into buffer
        while let Some(chunk_result) = data.next().await {
            let chunk = chunk_result.map_err(Error::Io)?;
            buffer.extend_from_slice(&chunk);
        }

        // Send entire data container as one USB transfer
        self.transport.send_bulk(&buffer).await?;

        // Receive response
        let response_bytes = self.transport.receive_bulk(512).await?;
        let response = ResponseContainer::from_bytes(&response_bytes)?;

        if response.transaction_id != tx_id {
            return Err(Error::invalid_data(format!(
                "Transaction ID mismatch: expected {}, got {}",
                tx_id, response.transaction_id
            )));
        }

        Ok(response)
    }

    /// Download an object as a stream of chunks.
    ///
    /// This is a convenience method that calls `execute_with_receive_stream`
    /// with GetObject operation.
    ///
    /// # Important
    ///
    /// The caller must consume the entire stream before calling any other
    /// session methods. The MTP session is locked while the stream is active.
    pub async fn get_object_stream(
        self: &Arc<Self>,
        handle: ObjectHandle,
    ) -> Result<ReceiveStream, Error> {
        self.execute_with_receive_stream(OperationCode::GetObject, &[handle.0])
            .await
    }

    /// Upload an object from a stream.
    ///
    /// This is a convenience method that streams object data directly to USB.
    ///
    /// # Arguments
    ///
    /// * `total_size` - Total bytes that will be sent
    /// * `data` - Stream of data chunks to send
    pub async fn send_object_stream<S>(&self, total_size: u64, data: S) -> Result<(), Error>
    where
        S: Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
    {
        let response = self
            .execute_with_send_stream(OperationCode::SendObject, &[], total_size, data)
            .await?;
        Self::check_response(&response, OperationCode::SendObject)?;
        Ok(())
    }
}

/// A stream of data chunks received from USB during a download operation.
///
/// This stream yields `Bytes` chunks as they arrive from the device,
/// allowing memory-efficient streaming without buffering the entire file.
///
/// # Important
///
/// The MTP session is locked while this stream exists. You must consume
/// the entire stream (or drop it) before calling other session methods.
pub struct ReceiveStream {
    /// The transport layer for USB communication.
    transport: Arc<dyn Transport>,
    /// Guard that holds the operation lock for the duration of streaming.
    _guard: OwnedMutexGuard<()>,
    /// Transaction ID for this operation.
    transaction_id: u32,
    /// Operation code for this operation.
    operation: OperationCode,
    /// Buffer for partial container data.
    buffer: Vec<u8>,
    /// Total length of current container (from header).
    container_length: usize,
    /// How much payload we've already yielded from current container.
    payload_yielded: usize,
    /// Whether we've parsed the container header.
    header_parsed: bool,
    /// Whether the stream is complete.
    done: bool,
}

impl ReceiveStream {
    /// Get the transaction ID for this operation.
    pub fn transaction_id(&self) -> u32 {
        self.transaction_id
    }

    /// Poll for the next chunk of data.
    ///
    /// This is the async version of the Stream trait's poll_next.
    pub async fn next_chunk(&mut self) -> Option<Result<Bytes, Error>> {
        if self.done {
            return None;
        }

        loop {
            // If we have buffered data beyond what we've already yielded, yield it
            if self.header_parsed {
                let payload_start = HEADER_SIZE + self.payload_yielded;
                let payload_end = std::cmp::min(self.buffer.len(), self.container_length);

                if payload_start < payload_end {
                    // We have new data to yield
                    let chunk_data = self.buffer[payload_start..payload_end].to_vec();
                    self.payload_yielded += chunk_data.len();

                    // Check if this container is complete
                    if self.buffer.len() >= self.container_length {
                        // Remove this container from buffer
                        self.buffer.drain(..self.container_length);
                        self.header_parsed = false;
                        self.container_length = 0;
                        self.payload_yielded = 0;
                    }

                    if !chunk_data.is_empty() {
                        return Some(Ok(Bytes::from(chunk_data)));
                    }
                } else if self.buffer.len() >= self.container_length {
                    // Container complete but no new data (shouldn't happen, but handle it)
                    self.buffer.drain(..self.container_length);
                    self.header_parsed = false;
                    self.container_length = 0;
                    self.payload_yielded = 0;
                }
            }

            // Need more data from USB
            match self.transport.receive_bulk(64 * 1024).await {
                Ok(bytes) => {
                    if bytes.is_empty() {
                        return Some(Err(Error::invalid_data("Empty response from device")));
                    }
                    self.buffer.extend_from_slice(&bytes);
                }
                Err(e) => {
                    self.done = true;
                    return Some(Err(e));
                }
            }

            // Try to parse container header if we haven't yet
            if !self.header_parsed && self.buffer.len() >= HEADER_SIZE {
                let ct = match container_type(&self.buffer) {
                    Ok(ct) => ct,
                    Err(e) => {
                        self.done = true;
                        return Some(Err(e));
                    }
                };

                match ct {
                    ContainerType::Data => {
                        let length = match unpack_u32(&self.buffer[0..4]) {
                            Ok(l) => l as usize,
                            Err(e) => {
                                self.done = true;
                                return Some(Err(e));
                            }
                        };
                        self.container_length = length;
                        self.header_parsed = true;
                    }
                    ContainerType::Response => {
                        // End of data transfer
                        let response = match ResponseContainer::from_bytes(&self.buffer) {
                            Ok(r) => r,
                            Err(e) => {
                                self.done = true;
                                return Some(Err(e));
                            }
                        };

                        self.done = true;

                        // Check transaction ID
                        if response.transaction_id != self.transaction_id {
                            return Some(Err(Error::invalid_data(format!(
                                "Transaction ID mismatch: expected {}, got {}",
                                self.transaction_id, response.transaction_id
                            ))));
                        }

                        // Check response code
                        if response.code != ResponseCode::Ok {
                            return Some(Err(Error::Protocol {
                                code: response.code,
                                operation: self.operation,
                            }));
                        }

                        return None;
                    }
                    _ => {
                        self.done = true;
                        return Some(Err(Error::invalid_data(format!(
                            "Unexpected container type: {:?}",
                            ct
                        ))));
                    }
                }
            }
        }
    }

    /// Collect all remaining data into a `Vec<u8>`.
    ///
    /// This consumes the stream and buffers all data in memory.
    pub async fn collect(mut self) -> Result<Vec<u8>, Error> {
        let mut data = Vec::new();
        while let Some(result) = self.next_chunk().await {
            let chunk = result?;
            data.extend_from_slice(&chunk);
        }
        Ok(data)
    }
}

/// Convert a ReceiveStream into a futures::Stream using async iteration.
///
/// This creates a proper Stream that can be used with StreamExt methods.
pub fn receive_stream_to_stream(recv: ReceiveStream) -> impl Stream<Item = Result<Bytes, Error>> {
    futures::stream::unfold(recv, |mut recv| async move {
        recv.next_chunk().await.map(|result| (result, recv))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ptp::{pack_u16, pack_u32, pack_u32_array, ContainerType};
    use crate::transport::mock::MockTransport;

    /// Create a mock transport as Arc<dyn Transport>.
    fn mock_transport() -> (Arc<dyn Transport>, Arc<MockTransport>) {
        let mock = Arc::new(MockTransport::new());
        let transport: Arc<dyn Transport> = Arc::clone(&mock) as Arc<dyn Transport>;
        (transport, mock)
    }

    /// Build an OK response container bytes.
    fn ok_response(tx_id: u32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(12);
        buf.extend_from_slice(&pack_u32(12)); // length
        buf.extend_from_slice(&pack_u16(ContainerType::Response.to_code()));
        buf.extend_from_slice(&pack_u16(ResponseCode::Ok.to_code()));
        buf.extend_from_slice(&pack_u32(tx_id));
        buf
    }

    /// Build a response container with params.
    fn response_with_params(tx_id: u32, code: ResponseCode, params: &[u32]) -> Vec<u8> {
        let len = 12 + params.len() * 4;
        let mut buf = Vec::with_capacity(len);
        buf.extend_from_slice(&pack_u32(len as u32));
        buf.extend_from_slice(&pack_u16(ContainerType::Response.to_code()));
        buf.extend_from_slice(&pack_u16(code.to_code()));
        buf.extend_from_slice(&pack_u32(tx_id));
        for p in params {
            buf.extend_from_slice(&pack_u32(*p));
        }
        buf
    }

    /// Build a data container.
    fn data_container(tx_id: u32, code: OperationCode, payload: &[u8]) -> Vec<u8> {
        let len = 12 + payload.len();
        let mut buf = Vec::with_capacity(len);
        buf.extend_from_slice(&pack_u32(len as u32));
        buf.extend_from_slice(&pack_u16(ContainerType::Data.to_code()));
        buf.extend_from_slice(&pack_u16(code.to_code()));
        buf.extend_from_slice(&pack_u32(tx_id));
        buf.extend_from_slice(payload);
        buf
    }

    #[tokio::test]
    async fn test_open_session() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1));

        let session = PtpSession::open(transport, 1).await.unwrap();
        assert_eq!(session.session_id(), SessionId(1));
    }

    #[tokio::test]
    async fn test_open_session_already_open_recovers() {
        let (transport, mock) = mock_transport();

        // First OpenSession returns SessionAlreadyOpen
        mock.queue_response(response_with_params(
            1,
            ResponseCode::SessionAlreadyOpen,
            &[],
        ));
        // CloseSession response (ignored, but we need to provide one)
        mock.queue_response(ok_response(2));
        // Second OpenSession (fresh session, tx_id starts at 1 again)
        mock.queue_response(ok_response(1));

        // Should succeed by closing and reopening
        let session = PtpSession::open(transport, 1).await.unwrap();
        assert_eq!(session.session_id(), SessionId(1));
    }

    #[tokio::test]
    async fn test_open_session_already_open_transaction_id_reset() {
        let (transport, mock) = mock_transport();

        // First OpenSession returns SessionAlreadyOpen
        mock.queue_response(response_with_params(
            1,
            ResponseCode::SessionAlreadyOpen,
            &[],
        ));
        // CloseSession response
        mock.queue_response(ok_response(2));
        // Second OpenSession (fresh session, tx_id starts at 1 again)
        mock.queue_response(ok_response(1));
        // Next operation should use tx_id = 2 (after the fresh OpenSession used 1)
        mock.queue_response(ok_response(2));

        let session = PtpSession::open(transport, 1).await.unwrap();

        // Perform an operation to verify transaction ID is properly reset
        // The next operation should use tx_id = 2 (since the fresh OpenSession used 1)
        session.delete_object(ObjectHandle(1)).await.unwrap();
    }

    #[tokio::test]
    async fn test_open_session_already_open_close_error_ignored() {
        let (transport, mock) = mock_transport();

        // First OpenSession returns SessionAlreadyOpen
        mock.queue_response(response_with_params(
            1,
            ResponseCode::SessionAlreadyOpen,
            &[],
        ));
        // CloseSession returns an error (should be ignored)
        mock.queue_response(response_with_params(2, ResponseCode::GeneralError, &[]));
        // Second OpenSession succeeds
        mock.queue_response(ok_response(1));

        // Should succeed even if CloseSession fails
        let session = PtpSession::open(transport, 1).await.unwrap();
        assert_eq!(session.session_id(), SessionId(1));
    }

    #[tokio::test]
    async fn test_open_session_error() {
        let (transport, mock) = mock_transport();
        mock.queue_response(response_with_params(1, ResponseCode::GeneralError, &[]));

        let result = PtpSession::open(transport, 1).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_storage_ids() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        // GetStorageIds data response
        let storage_ids_data = pack_u32_array(&[0x00010001, 0x00010002]);
        mock.queue_response(data_container(
            2,
            OperationCode::GetStorageIds,
            &storage_ids_data,
        ));
        mock.queue_response(ok_response(2));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let ids = session.get_storage_ids().await.unwrap();

        assert_eq!(ids, vec![StorageId(0x00010001), StorageId(0x00010002)]);
    }

    #[tokio::test]
    async fn test_get_object_handles() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        // GetObjectHandles data response
        let handles_data = pack_u32_array(&[1, 2, 3]);
        mock.queue_response(data_container(
            2,
            OperationCode::GetObjectHandles,
            &handles_data,
        ));
        mock.queue_response(ok_response(2));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let handles = session
            .get_object_handles(StorageId::ALL, None, None)
            .await
            .unwrap();

        assert_eq!(
            handles,
            vec![ObjectHandle(1), ObjectHandle(2), ObjectHandle(3)]
        );
    }

    #[tokio::test]
    async fn test_get_object() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        // GetObject data response
        let object_data = vec![0x01, 0x02, 0x03, 0x04, 0x05];
        mock.queue_response(data_container(2, OperationCode::GetObject, &object_data));
        mock.queue_response(ok_response(2));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let data = session.get_object(ObjectHandle(1)).await.unwrap();

        assert_eq!(data, object_data);
    }

    #[tokio::test]
    async fn test_delete_object() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(ok_response(2)); // DeleteObject

        let session = PtpSession::open(transport, 1).await.unwrap();
        session.delete_object(ObjectHandle(1)).await.unwrap();
    }

    #[tokio::test]
    async fn test_transaction_id_increment() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(ok_response(2)); // First operation
        mock.queue_response(ok_response(3)); // Second operation

        let session = PtpSession::open(transport, 1).await.unwrap();

        // Execute two operations and verify transaction IDs increment
        session.delete_object(ObjectHandle(1)).await.unwrap();
        session.delete_object(ObjectHandle(2)).await.unwrap();
    }

    #[tokio::test]
    async fn test_transaction_id_mismatch() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(ok_response(999)); // Wrong transaction ID

        let session = PtpSession::open(transport, 1).await.unwrap();
        let result = session.delete_object(ObjectHandle(1)).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_copy_object() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(response_with_params(2, ResponseCode::Ok, &[100])); // CopyObject with new handle

        let session = PtpSession::open(transport, 1).await.unwrap();
        let new_handle = session
            .copy_object(ObjectHandle(1), StorageId(0x00010001), ObjectHandle::ROOT)
            .await
            .unwrap();

        assert_eq!(new_handle, ObjectHandle(100));
    }

    #[tokio::test]
    async fn test_close_session() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(ok_response(2)); // CloseSession

        let session = PtpSession::open(transport, 1).await.unwrap();
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_close_session_ignores_errors() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(response_with_params(2, ResponseCode::GeneralError, &[])); // CloseSession error

        let session = PtpSession::open(transport, 1).await.unwrap();
        // Should succeed even if close fails
        session.close().await.unwrap();
    }

    // =========================================================================
    // Event polling tests
    // =========================================================================

    fn event_container(code: u16, params: [u32; 3]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(24);
        buf.extend_from_slice(&pack_u32(24)); // length = 24
        buf.extend_from_slice(&pack_u16(ContainerType::Event.to_code()));
        buf.extend_from_slice(&pack_u16(code));
        buf.extend_from_slice(&pack_u32(0)); // transaction_id
        buf.extend_from_slice(&pack_u32(params[0]));
        buf.extend_from_slice(&pack_u32(params[1]));
        buf.extend_from_slice(&pack_u32(params[2]));
        buf
    }

    #[tokio::test]
    async fn test_poll_event_object_added() {
        use crate::ptp::EventCode;

        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        // Queue an ObjectAdded event (code 0x4002)
        mock.queue_interrupt(event_container(0x4002, [42, 0, 0]));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let event = session.poll_event().await.unwrap().unwrap();

        assert_eq!(event.code, EventCode::ObjectAdded);
        assert_eq!(event.params[0], 42);
    }

    #[tokio::test]
    async fn test_poll_event_store_removed() {
        use crate::ptp::EventCode;

        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        // Queue a StoreRemoved event (code 0x4005)
        mock.queue_interrupt(event_container(0x4005, [0x00010001, 0, 0]));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let event = session.poll_event().await.unwrap().unwrap();

        assert_eq!(event.code, EventCode::StoreRemoved);
        assert_eq!(event.params[0], 0x00010001);
    }

    #[tokio::test]
    async fn test_poll_event_multiple_events() {
        use crate::ptp::EventCode;

        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        // Queue multiple events
        mock.queue_interrupt(event_container(0x4002, [1, 0, 0])); // ObjectAdded
        mock.queue_interrupt(event_container(0x4002, [2, 0, 0])); // ObjectAdded
        mock.queue_interrupt(event_container(0x4003, [1, 0, 0])); // ObjectRemoved

        let session = PtpSession::open(transport, 1).await.unwrap();

        let event1 = session.poll_event().await.unwrap().unwrap();
        assert_eq!(event1.code, EventCode::ObjectAdded);
        assert_eq!(event1.params[0], 1);

        let event2 = session.poll_event().await.unwrap().unwrap();
        assert_eq!(event2.code, EventCode::ObjectAdded);
        assert_eq!(event2.params[0], 2);

        let event3 = session.poll_event().await.unwrap().unwrap();
        assert_eq!(event3.code, EventCode::ObjectRemoved);
        assert_eq!(event3.params[0], 1);
    }

    // =========================================================================
    // Object property and rename tests
    // =========================================================================

    #[tokio::test]
    async fn test_get_object_prop_value() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        // GetObjectPropValue data response (property value is raw bytes)
        let prop_value = vec![0x05, 0x48, 0x00, 0x69, 0x00, 0x00, 0x00]; // Packed string "Hi"
        mock.queue_response(data_container(
            2,
            OperationCode::GetObjectPropValue,
            &prop_value,
        ));
        mock.queue_response(ok_response(2));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let data = session
            .get_object_prop_value(ObjectHandle(1), ObjectPropertyCode::ObjectFileName)
            .await
            .unwrap();

        assert_eq!(data, prop_value);
    }

    #[tokio::test]
    async fn test_set_object_prop_value() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(ok_response(2)); // SetObjectPropValue

        let session = PtpSession::open(transport, 1).await.unwrap();
        let prop_value = pack_string("newfile.txt");
        session
            .set_object_prop_value(
                ObjectHandle(1),
                ObjectPropertyCode::ObjectFileName,
                &prop_value,
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_set_object_prop_value_not_supported() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(response_with_params(
            2,
            ResponseCode::OperationNotSupported,
            &[],
        ));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let prop_value = pack_string("newfile.txt");
        let result = session
            .set_object_prop_value(
                ObjectHandle(1),
                ObjectPropertyCode::ObjectFileName,
                &prop_value,
            )
            .await;

        assert!(matches!(
            result,
            Err(crate::Error::Protocol {
                code: ResponseCode::OperationNotSupported,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn test_rename_object() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(ok_response(2)); // SetObjectPropValue (for rename)

        let session = PtpSession::open(transport, 1).await.unwrap();
        session
            .rename_object(ObjectHandle(1), "renamed.txt")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_rename_object_not_supported() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(response_with_params(
            2,
            ResponseCode::OperationNotSupported,
            &[],
        ));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let result = session.rename_object(ObjectHandle(1), "renamed.txt").await;

        assert!(matches!(
            result,
            Err(crate::Error::Protocol {
                code: ResponseCode::OperationNotSupported,
                ..
            })
        ));
    }

    // =========================================================================
    // Device property tests
    // =========================================================================

    /// Build a battery level property descriptor for testing.
    fn build_battery_prop_desc(current: u8) -> Vec<u8> {
        let mut buf = Vec::new();
        // PropertyCode: 0x5001 (BatteryLevel)
        buf.extend_from_slice(&pack_u16(0x5001));
        // DataType: UINT8 (0x0002)
        buf.extend_from_slice(&pack_u16(0x0002));
        // GetSet: read-only (0x00)
        buf.push(0x00);
        // DefaultValue: 100
        buf.push(100);
        // CurrentValue
        buf.push(current);
        // FormFlag: Range (0x01)
        buf.push(0x01);
        // Range: min=0, max=100, step=1
        buf.push(0); // min
        buf.push(100); // max
        buf.push(1); // step
        buf
    }

    #[tokio::test]
    async fn test_get_device_prop_desc() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        // Queue battery level prop desc
        let prop_desc_data = build_battery_prop_desc(75);
        mock.queue_response(data_container(
            2,
            OperationCode::GetDevicePropDesc,
            &prop_desc_data,
        ));
        mock.queue_response(ok_response(2));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let desc = session
            .get_device_prop_desc(DevicePropertyCode::BatteryLevel)
            .await
            .unwrap();

        assert_eq!(desc.property_code, DevicePropertyCode::BatteryLevel);
        assert_eq!(desc.data_type, PropertyDataType::Uint8);
        assert!(!desc.writable);
        assert_eq!(desc.current_value, PropertyValue::Uint8(75));
    }

    #[tokio::test]
    async fn test_get_device_prop_desc_not_supported() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(response_with_params(
            2,
            ResponseCode::DevicePropNotSupported,
            &[],
        ));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let result = session
            .get_device_prop_desc(DevicePropertyCode::BatteryLevel)
            .await;

        assert!(matches!(
            result,
            Err(crate::Error::Protocol {
                code: ResponseCode::DevicePropNotSupported,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn test_get_device_prop_value() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        // Queue battery level value (75%)
        let value_data = vec![75u8];
        mock.queue_response(data_container(
            2,
            OperationCode::GetDevicePropValue,
            &value_data,
        ));
        mock.queue_response(ok_response(2));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let data = session
            .get_device_prop_value(DevicePropertyCode::BatteryLevel)
            .await
            .unwrap();

        assert_eq!(data, vec![75u8]);
    }

    #[tokio::test]
    async fn test_get_device_prop_value_typed() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        // Queue ISO value (400 = 0x0190)
        let value_data = vec![0x90, 0x01]; // 400 in little-endian
        mock.queue_response(data_container(
            2,
            OperationCode::GetDevicePropValue,
            &value_data,
        ));
        mock.queue_response(ok_response(2));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let value = session
            .get_device_prop_value_typed(
                DevicePropertyCode::ExposureIndex,
                PropertyDataType::Uint16,
            )
            .await
            .unwrap();

        assert_eq!(value, PropertyValue::Uint16(400));
    }

    #[tokio::test]
    async fn test_set_device_prop_value() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(ok_response(2)); // SetDevicePropValue

        let session = PtpSession::open(transport, 1).await.unwrap();
        let value = vec![0x90, 0x01]; // 400 in little-endian
        session
            .set_device_prop_value(DevicePropertyCode::ExposureIndex, &value)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_set_device_prop_value_typed() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(ok_response(2)); // SetDevicePropValue

        let session = PtpSession::open(transport, 1).await.unwrap();
        session
            .set_device_prop_value_typed(
                DevicePropertyCode::ExposureIndex,
                &PropertyValue::Uint16(400),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_set_device_prop_value_invalid() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(response_with_params(
            2,
            ResponseCode::InvalidDevicePropValue,
            &[],
        ));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let result = session
            .set_device_prop_value(DevicePropertyCode::ExposureIndex, &[0x00, 0x00])
            .await;

        assert!(matches!(
            result,
            Err(crate::Error::Protocol {
                code: ResponseCode::InvalidDevicePropValue,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn test_reset_device_prop_value() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(ok_response(2)); // ResetDevicePropValue

        let session = PtpSession::open(transport, 1).await.unwrap();
        session
            .reset_device_prop_value(DevicePropertyCode::ExposureIndex)
            .await
            .unwrap();
    }

    // =========================================================================
    // Capture tests
    // =========================================================================

    #[tokio::test]
    async fn test_initiate_capture() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(ok_response(2)); // InitiateCapture

        let session = PtpSession::open(transport, 1).await.unwrap();
        session
            .initiate_capture(StorageId(0), ObjectFormatCode::Undefined)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_initiate_capture_with_format() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(ok_response(2)); // InitiateCapture

        let session = PtpSession::open(transport, 1).await.unwrap();
        session
            .initiate_capture(StorageId(0x00010001), ObjectFormatCode::Jpeg)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_initiate_capture_not_supported() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(response_with_params(
            2,
            ResponseCode::OperationNotSupported,
            &[],
        ));

        let session = PtpSession::open(transport, 1).await.unwrap();
        let result = session
            .initiate_capture(StorageId(0), ObjectFormatCode::Undefined)
            .await;

        assert!(matches!(
            result,
            Err(crate::Error::Protocol {
                code: ResponseCode::OperationNotSupported,
                ..
            })
        ));
    }

    // =========================================================================
    // Streaming tests
    // =========================================================================

    #[tokio::test]
    async fn test_receive_stream_small_file() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        // GetObject data response (small file fits in one container)
        let file_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        mock.queue_response(data_container(2, OperationCode::GetObject, &file_data));
        mock.queue_response(ok_response(2));

        let session = Arc::new(PtpSession::open(transport, 1).await.unwrap());

        // Use streaming API
        let mut stream = session.get_object_stream(ObjectHandle(1)).await.unwrap();

        // Collect all chunks
        let mut received = Vec::new();
        while let Some(result) = stream.next_chunk().await {
            let chunk = result.unwrap();
            received.extend_from_slice(&chunk);
        }

        assert_eq!(received, file_data);
    }

    #[tokio::test]
    async fn test_receive_stream_collect() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        let file_data = vec![1, 2, 3, 4, 5];
        mock.queue_response(data_container(2, OperationCode::GetObject, &file_data));
        mock.queue_response(ok_response(2));

        let session = Arc::new(PtpSession::open(transport, 1).await.unwrap());

        let stream = session.get_object_stream(ObjectHandle(1)).await.unwrap();
        let collected = stream.collect().await.unwrap();

        assert_eq!(collected, file_data);
    }

    #[tokio::test]
    async fn test_receive_stream_error_response() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        // Return error response instead of data
        mock.queue_response(response_with_params(
            2,
            ResponseCode::InvalidObjectHandle,
            &[],
        ));

        let session = Arc::new(PtpSession::open(transport, 1).await.unwrap());

        let mut stream = session.get_object_stream(ObjectHandle(999)).await.unwrap();

        // Should get error when reading
        let result = stream.next_chunk().await;
        assert!(result.is_some());
        let err = result.unwrap();
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_send_stream_small_file() {
        use futures::stream;

        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(ok_response(2)); // SendObject response

        let session = PtpSession::open(transport, 1).await.unwrap();

        // Create a small data stream (use iter instead of once for Unpin)
        let data = vec![1u8, 2, 3, 4, 5];
        let data_stream = stream::iter(vec![Ok::<_, std::io::Error>(Bytes::from(data.clone()))]);

        // Send using streaming API
        session.send_object_stream(5, data_stream).await.unwrap();
    }

    #[tokio::test]
    async fn test_send_stream_multiple_chunks() {
        use futures::stream;

        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession
        mock.queue_response(ok_response(2)); // SendObject response

        let session = PtpSession::open(transport, 1).await.unwrap();

        // Create a multi-chunk data stream
        let chunks = vec![
            Ok::<_, std::io::Error>(Bytes::from(vec![1, 2, 3])),
            Ok(Bytes::from(vec![4, 5, 6])),
            Ok(Bytes::from(vec![7, 8, 9, 10])),
        ];
        let data_stream = stream::iter(chunks);

        // Send using streaming API (total size = 10)
        session.send_object_stream(10, data_stream).await.unwrap();
    }

    #[tokio::test]
    async fn test_receive_stream_to_stream_conversion() {
        let (transport, mock) = mock_transport();
        mock.queue_response(ok_response(1)); // OpenSession

        let file_data = vec![1, 2, 3, 4, 5];
        mock.queue_response(data_container(2, OperationCode::GetObject, &file_data));
        mock.queue_response(ok_response(2));

        let session = Arc::new(PtpSession::open(transport, 1).await.unwrap());

        let recv_stream = session.get_object_stream(ObjectHandle(1)).await.unwrap();

        // Convert to futures::Stream and use StreamExt
        // Use pin_mut! to make it Unpin
        use futures::StreamExt;
        use std::pin::pin;
        let mut stream = pin!(receive_stream_to_stream(recv_stream));

        let mut collected = Vec::new();
        while let Some(result) = stream.next().await {
            collected.extend_from_slice(&result.unwrap());
        }

        assert_eq!(collected, file_data);
    }
}
