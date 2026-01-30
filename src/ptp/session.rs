//! PTP session management.
//!
//! This module provides session-level operations for MTP/PTP communication.
//! A session maintains the connection state and serializes concurrent operations.

use crate::ptp::{
    container_type, unpack_u32, unpack_u32_array, CommandContainer, ContainerType, DataContainer,
    DeviceInfo, EventContainer, ObjectFormatCode, ObjectHandle, ObjectInfo, OperationCode,
    ResponseCode, ResponseContainer, SessionId, StorageId, StorageInfo, TransactionId,
};
use crate::transport::Transport;
use crate::Error;
use futures::lock::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

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
    operation_lock: Mutex<()>,
}

impl PtpSession {
    /// Create a new session (internal, use open() to start session).
    fn new(transport: Arc<dyn Transport>, session_id: SessionId) -> Self {
        Self {
            transport,
            session_id,
            transaction_id: AtomicU32::new(TransactionId::FIRST.0),
            operation_lock: Mutex::new(()),
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

        if response.code != ResponseCode::Ok && response.code != ResponseCode::SessionAlreadyOpen {
            return Err(Error::Protocol {
                code: response.code,
                operation: OperationCode::OpenSession,
            });
        }

        Ok(session)
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
        Self::check_response(response, OperationCode::GetDeviceInfo)?;
        DeviceInfo::from_bytes(&data)
    }

    /// Get storage IDs.
    ///
    /// Returns a list of storage IDs available on the device.
    pub async fn get_storage_ids(&self) -> Result<Vec<StorageId>, Error> {
        let (response, data) = self
            .execute_with_receive(OperationCode::GetStorageIds, &[])
            .await?;
        Self::check_response(response, OperationCode::GetStorageIds)?;
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
        Self::check_response(response, OperationCode::GetStorageInfo)?;
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
        Self::check_response(response, OperationCode::GetObjectHandles)?;
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
        Self::check_response(response, OperationCode::GetObjectInfo)?;
        ObjectInfo::from_bytes(&data)
    }

    /// Get object (download).
    ///
    /// Downloads the complete data of an object.
    pub async fn get_object(&self, handle: ObjectHandle) -> Result<Vec<u8>, Error> {
        let (response, data) = self
            .execute_with_receive(OperationCode::GetObject, &[handle.0])
            .await?;
        Self::check_response(response, OperationCode::GetObject)?;
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
        Self::check_response(response, OperationCode::GetPartialObject)?;
        Ok(data)
    }

    /// Get thumbnail.
    ///
    /// Downloads the thumbnail image for an object.
    pub async fn get_thumb(&self, handle: ObjectHandle) -> Result<Vec<u8>, Error> {
        let (response, data) = self
            .execute_with_receive(OperationCode::GetThumb, &[handle.0])
            .await?;
        Self::check_response(response, OperationCode::GetThumb)?;
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
        let data = info.to_bytes();
        let response = self
            .execute_with_send(
                OperationCode::SendObjectInfo,
                &[storage_id.0, parent.0],
                &data,
            )
            .await?;
        Self::check_response(response.clone(), OperationCode::SendObjectInfo)?;

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
        Self::check_response(response, OperationCode::SendObject)?;
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
        Self::check_response(response, OperationCode::DeleteObject)?;
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
        Self::check_response(response, OperationCode::MoveObject)?;
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
        Self::check_response(response.clone(), OperationCode::CopyObject)?;

        if response.params.is_empty() {
            return Err(Error::invalid_data("CopyObject response missing handle"));
        }
        Ok(ObjectHandle(response.params[0]))
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
    fn check_response(response: ResponseContainer, operation: OperationCode) -> Result<(), Error> {
        if response.code == ResponseCode::Ok {
            Ok(())
        } else {
            Err(Error::Protocol {
                code: response.code,
                operation,
            })
        }
    }
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
    async fn test_open_session_already_open() {
        let (transport, mock) = mock_transport();
        mock.queue_response(response_with_params(
            1,
            ResponseCode::SessionAlreadyOpen,
            &[],
        ));

        // Should succeed even if session is already open
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
}
