//! MTP/PTP USB container format.
//!
//! This module implements the USB container format used for MTP/PTP communication.
//! All containers share a common 12-byte header followed by optional parameters or payload.
//!
//! ## Container format (little-endian)
//!
//! Header (12 bytes):
//! - Offset 0: Length (u32) - Total container size including header
//! - Offset 4: Type (u16) - Container type
//! - Offset 6: Code (u16) - Operation/Response/Event code
//! - Offset 8: TransactionID (u32)
//!
//! After header: parameters (each u32) or payload bytes.

use super::{pack_u16, pack_u32, unpack_u16, unpack_u32, EventCode, OperationCode, ResponseCode};

/// Minimum container header size in bytes.
const HEADER_SIZE: usize = 12;

/// Container type identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ContainerType {
    /// Command container (sent to device).
    Command = 1,
    /// Data container (bidirectional).
    Data = 2,
    /// Response container (from device).
    Response = 3,
    /// Event container (from device).
    Event = 4,
}

impl ContainerType {
    /// Convert a raw u16 value to a ContainerType.
    pub fn from_code(code: u16) -> Option<Self> {
        match code {
            1 => Some(ContainerType::Command),
            2 => Some(ContainerType::Data),
            3 => Some(ContainerType::Response),
            4 => Some(ContainerType::Event),
            _ => None,
        }
    }

    /// Convert a ContainerType to its raw u16 value.
    pub fn to_code(self) -> u16 {
        self as u16
    }
}

/// Determine the container type from a raw buffer.
///
/// Returns an error if the buffer is too small or contains an invalid container type.
pub fn container_type(buf: &[u8]) -> Result<ContainerType, crate::Error> {
    if buf.len() < HEADER_SIZE {
        return Err(crate::Error::invalid_data(format!(
            "container too small: need at least {} bytes, have {}",
            HEADER_SIZE,
            buf.len()
        )));
    }

    let type_code = unpack_u16(&buf[4..6])?;
    ContainerType::from_code(type_code)
        .ok_or_else(|| crate::Error::invalid_data(format!("invalid container type: {}", type_code)))
}

/// Command container sent to the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandContainer {
    /// Operation code for the command.
    pub code: OperationCode,
    /// Transaction ID for this operation.
    pub transaction_id: u32,
    /// Parameters (0-5 u32 values).
    pub params: Vec<u32>,
}

impl CommandContainer {
    /// Serialize the command container to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let param_bytes = self.params.len() * 4;
        let total_len = HEADER_SIZE + param_bytes;

        let mut buf = Vec::with_capacity(total_len);

        // Header
        buf.extend_from_slice(&pack_u32(total_len as u32));
        buf.extend_from_slice(&pack_u16(ContainerType::Command.to_code()));
        buf.extend_from_slice(&pack_u16(self.code.to_code()));
        buf.extend_from_slice(&pack_u32(self.transaction_id));

        // Parameters
        for &param in &self.params {
            buf.extend_from_slice(&pack_u32(param));
        }

        buf
    }
}

/// Data container for transferring payload data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataContainer {
    /// Operation code this data belongs to.
    pub code: OperationCode,
    /// Transaction ID for this operation.
    pub transaction_id: u32,
    /// Payload bytes.
    pub payload: Vec<u8>,
}

impl DataContainer {
    /// Serialize the data container to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let total_len = HEADER_SIZE + self.payload.len();

        let mut buf = Vec::with_capacity(total_len);

        // Header
        buf.extend_from_slice(&pack_u32(total_len as u32));
        buf.extend_from_slice(&pack_u16(ContainerType::Data.to_code()));
        buf.extend_from_slice(&pack_u16(self.code.to_code()));
        buf.extend_from_slice(&pack_u32(self.transaction_id));

        // Payload
        buf.extend_from_slice(&self.payload);

        buf
    }

    /// Parse a data container from bytes.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::Error> {
        if buf.len() < HEADER_SIZE {
            return Err(crate::Error::invalid_data(format!(
                "data container too small: need at least {} bytes, have {}",
                HEADER_SIZE,
                buf.len()
            )));
        }

        let length = unpack_u32(&buf[0..4])? as usize;
        let type_code = unpack_u16(&buf[4..6])?;
        let code = unpack_u16(&buf[6..8])?;
        let transaction_id = unpack_u32(&buf[8..12])?;

        // Validate container type
        if type_code != ContainerType::Data.to_code() {
            return Err(crate::Error::invalid_data(format!(
                "expected Data container type ({}), got {}",
                ContainerType::Data.to_code(),
                type_code
            )));
        }

        // Validate length
        if buf.len() < length {
            return Err(crate::Error::invalid_data(format!(
                "data container length mismatch: header says {}, have {}",
                length,
                buf.len()
            )));
        }

        // Extract payload
        let payload = buf[HEADER_SIZE..length].to_vec();

        Ok(DataContainer {
            code: OperationCode::from_code(code),
            transaction_id,
            payload,
        })
    }
}

/// Response container from the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseContainer {
    /// Response code indicating success or failure.
    pub code: ResponseCode,
    /// Transaction ID this response corresponds to.
    pub transaction_id: u32,
    /// Response parameters (0-5 u32 values).
    pub params: Vec<u32>,
}

impl ResponseContainer {
    /// Parse a response container from bytes.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::Error> {
        if buf.len() < HEADER_SIZE {
            return Err(crate::Error::invalid_data(format!(
                "response container too small: need at least {} bytes, have {}",
                HEADER_SIZE,
                buf.len()
            )));
        }

        let length = unpack_u32(&buf[0..4])? as usize;
        let type_code = unpack_u16(&buf[4..6])?;
        let code = unpack_u16(&buf[6..8])?;
        let transaction_id = unpack_u32(&buf[8..12])?;

        // Validate container type
        if type_code != ContainerType::Response.to_code() {
            return Err(crate::Error::invalid_data(format!(
                "expected Response container type ({}), got {}",
                ContainerType::Response.to_code(),
                type_code
            )));
        }

        // Validate length
        if buf.len() < length {
            return Err(crate::Error::invalid_data(format!(
                "response container length mismatch: header says {}, have {}",
                length,
                buf.len()
            )));
        }

        // Parse parameters
        let param_bytes = length - HEADER_SIZE;
        if param_bytes % 4 != 0 {
            return Err(crate::Error::invalid_data(format!(
                "response parameter bytes not aligned: {} bytes",
                param_bytes
            )));
        }

        let param_count = param_bytes / 4;
        let mut params = Vec::with_capacity(param_count);
        for i in 0..param_count {
            let offset = HEADER_SIZE + i * 4;
            params.push(unpack_u32(&buf[offset..])?);
        }

        Ok(ResponseContainer {
            code: ResponseCode::from_code(code),
            transaction_id,
            params,
        })
    }

    /// Check if the response indicates success (Ok).
    pub fn is_ok(&self) -> bool {
        self.code == ResponseCode::Ok
    }
}

/// Event container from the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventContainer {
    /// Event code identifying the event type.
    pub code: EventCode,
    /// Transaction ID (may be 0 for unsolicited events).
    pub transaction_id: u32,
    /// Event parameters (always exactly 3).
    pub params: [u32; 3],
}

impl EventContainer {
    /// Parse an event container from bytes.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::Error> {
        // Events always have 3 parameters, so minimum size is 12 + 12 = 24 bytes
        const EVENT_SIZE: usize = HEADER_SIZE + 12;

        if buf.len() < HEADER_SIZE {
            return Err(crate::Error::invalid_data(format!(
                "event container too small: need at least {} bytes, have {}",
                HEADER_SIZE,
                buf.len()
            )));
        }

        let length = unpack_u32(&buf[0..4])? as usize;
        let type_code = unpack_u16(&buf[4..6])?;
        let code = unpack_u16(&buf[6..8])?;
        let transaction_id = unpack_u32(&buf[8..12])?;

        // Validate container type
        if type_code != ContainerType::Event.to_code() {
            return Err(crate::Error::invalid_data(format!(
                "expected Event container type ({}), got {}",
                ContainerType::Event.to_code(),
                type_code
            )));
        }

        // Validate length for event (must have exactly 3 parameters)
        if length != EVENT_SIZE {
            return Err(crate::Error::invalid_data(format!(
                "event container wrong size: expected {}, got {}",
                EVENT_SIZE, length
            )));
        }

        if buf.len() < EVENT_SIZE {
            return Err(crate::Error::invalid_data(format!(
                "event container buffer too small: need {}, have {}",
                EVENT_SIZE,
                buf.len()
            )));
        }

        // Parse the 3 parameters
        let param1 = unpack_u32(&buf[12..16])?;
        let param2 = unpack_u32(&buf[16..20])?;
        let param3 = unpack_u32(&buf[20..24])?;

        Ok(EventContainer {
            code: EventCode::from_code(code),
            transaction_id,
            params: [param1, param2, param3],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // ContainerType tests
    // =========================================================================

    #[test]
    fn container_type_from_code() {
        assert_eq!(ContainerType::from_code(1), Some(ContainerType::Command));
        assert_eq!(ContainerType::from_code(2), Some(ContainerType::Data));
        assert_eq!(ContainerType::from_code(3), Some(ContainerType::Response));
        assert_eq!(ContainerType::from_code(4), Some(ContainerType::Event));
        assert_eq!(ContainerType::from_code(0), None);
        assert_eq!(ContainerType::from_code(5), None);
        assert_eq!(ContainerType::from_code(0xFFFF), None);
    }

    #[test]
    fn container_type_to_code() {
        assert_eq!(ContainerType::Command.to_code(), 1);
        assert_eq!(ContainerType::Data.to_code(), 2);
        assert_eq!(ContainerType::Response.to_code(), 3);
        assert_eq!(ContainerType::Event.to_code(), 4);
    }

    #[test]
    fn container_type_roundtrip() {
        for ct in [
            ContainerType::Command,
            ContainerType::Data,
            ContainerType::Response,
            ContainerType::Event,
        ] {
            assert_eq!(ContainerType::from_code(ct.to_code()), Some(ct));
        }
    }

    // =========================================================================
    // container_type() function tests
    // =========================================================================

    #[test]
    fn container_type_function_command() {
        let bytes = vec![
            0x0C, 0x00, 0x00, 0x00, // length = 12
            0x01, 0x00, // type = Command
            0x03, 0x10, // code
            0x05, 0x00, 0x00, 0x00, // transaction_id
        ];
        assert_eq!(container_type(&bytes).unwrap(), ContainerType::Command);
    }

    #[test]
    fn container_type_function_data() {
        let bytes = vec![
            0x0C, 0x00, 0x00, 0x00, // length = 12
            0x02, 0x00, // type = Data
            0x04, 0x10, // code
            0x02, 0x00, 0x00, 0x00, // transaction_id
        ];
        assert_eq!(container_type(&bytes).unwrap(), ContainerType::Data);
    }

    #[test]
    fn container_type_function_response() {
        let bytes = vec![
            0x0C, 0x00, 0x00, 0x00, // length = 12
            0x03, 0x00, // type = Response
            0x01, 0x20, // code
            0x01, 0x00, 0x00, 0x00, // transaction_id
        ];
        assert_eq!(container_type(&bytes).unwrap(), ContainerType::Response);
    }

    #[test]
    fn container_type_function_event() {
        let bytes = vec![
            0x18, 0x00, 0x00, 0x00, // length = 24
            0x04, 0x00, // type = Event
            0x02, 0x40, // code
            0x00, 0x00, 0x00, 0x00, // transaction_id
            0x00, 0x00, 0x00, 0x00, // param1
            0x00, 0x00, 0x00, 0x00, // param2
            0x00, 0x00, 0x00, 0x00, // param3
        ];
        assert_eq!(container_type(&bytes).unwrap(), ContainerType::Event);
    }

    #[test]
    fn container_type_function_insufficient_bytes() {
        assert!(container_type(&[]).is_err());
        assert!(container_type(&[0x00; 11]).is_err());
    }

    #[test]
    fn container_type_function_invalid_type() {
        let bytes = vec![
            0x0C, 0x00, 0x00, 0x00, // length = 12
            0x00, 0x00, // type = invalid (0)
            0x01, 0x20, // code
            0x01, 0x00, 0x00, 0x00, // transaction_id
        ];
        assert!(container_type(&bytes).is_err());

        let bytes = vec![
            0x0C, 0x00, 0x00, 0x00, // length = 12
            0x05, 0x00, // type = invalid (5)
            0x01, 0x20, // code
            0x01, 0x00, 0x00, 0x00, // transaction_id
        ];
        assert!(container_type(&bytes).is_err());
    }

    // =========================================================================
    // CommandContainer tests
    // =========================================================================

    #[test]
    fn command_container_no_params() {
        let cmd = CommandContainer {
            code: OperationCode::CloseSession,
            transaction_id: 5,
            params: vec![],
        };
        let bytes = cmd.to_bytes();
        assert_eq!(bytes.len(), 12);
        assert_eq!(&bytes[0..4], &[0x0C, 0x00, 0x00, 0x00]); // length = 12
        assert_eq!(&bytes[4..6], &[0x01, 0x00]); // type = Command
        assert_eq!(&bytes[6..8], &[0x03, 0x10]); // code = CloseSession (0x1003)
        assert_eq!(&bytes[8..12], &[0x05, 0x00, 0x00, 0x00]); // transaction_id = 5
    }

    #[test]
    fn command_container_one_param() {
        let cmd = CommandContainer {
            code: OperationCode::OpenSession,
            transaction_id: 1,
            params: vec![1],
        };
        let bytes = cmd.to_bytes();
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[0..4], &[0x10, 0x00, 0x00, 0x00]); // length = 16
        assert_eq!(&bytes[12..16], &[0x01, 0x00, 0x00, 0x00]); // param1 = 1
    }

    #[test]
    fn command_container_multiple_params() {
        let cmd = CommandContainer {
            code: OperationCode::GetObjectHandles,
            transaction_id: 10,
            params: vec![0x00010001, 0x00000000, 0xFFFFFFFF],
        };
        let bytes = cmd.to_bytes();
        assert_eq!(bytes.len(), 24); // 12 header + 12 params
        assert_eq!(&bytes[0..4], &[0x18, 0x00, 0x00, 0x00]); // length = 24
        assert_eq!(&bytes[4..6], &[0x01, 0x00]); // type = Command
        assert_eq!(&bytes[6..8], &[0x07, 0x10]); // code = GetObjectHandles (0x1007)
        assert_eq!(&bytes[8..12], &[0x0A, 0x00, 0x00, 0x00]); // transaction_id = 10
        assert_eq!(&bytes[12..16], &[0x01, 0x00, 0x01, 0x00]); // param1 = 0x00010001
        assert_eq!(&bytes[16..20], &[0x00, 0x00, 0x00, 0x00]); // param2 = 0
        assert_eq!(&bytes[20..24], &[0xFF, 0xFF, 0xFF, 0xFF]); // param3 = 0xFFFFFFFF
    }

    #[test]
    fn command_container_five_params() {
        let cmd = CommandContainer {
            code: OperationCode::GetPartialObject,
            transaction_id: 42,
            params: vec![1, 2, 3, 4, 5],
        };
        let bytes = cmd.to_bytes();
        assert_eq!(bytes.len(), 32); // 12 header + 20 params
        assert_eq!(&bytes[0..4], &[0x20, 0x00, 0x00, 0x00]); // length = 32
    }

    // =========================================================================
    // DataContainer tests
    // =========================================================================

    #[test]
    fn data_container_to_bytes() {
        let data = DataContainer {
            code: OperationCode::GetStorageIds,
            transaction_id: 2,
            payload: vec![0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00],
        };
        let bytes = data.to_bytes();
        assert_eq!(bytes.len(), 20); // 12 header + 8 payload
        assert_eq!(&bytes[0..4], &[0x14, 0x00, 0x00, 0x00]); // length = 20
        assert_eq!(&bytes[4..6], &[0x02, 0x00]); // type = Data
        assert_eq!(&bytes[6..8], &[0x04, 0x10]); // code = GetStorageIds (0x1004)
        assert_eq!(&bytes[8..12], &[0x02, 0x00, 0x00, 0x00]); // transaction_id = 2
        assert_eq!(
            &bytes[12..20],
            &[0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00]
        ); // payload
    }

    #[test]
    fn data_container_parse() {
        let bytes = vec![
            0x14, 0x00, 0x00, 0x00, // length = 20
            0x02, 0x00, // type = Data
            0x04, 0x10, // code = GetStorageIds (0x1004)
            0x02, 0x00, 0x00, 0x00, // transaction_id = 2
            0x01, 0x00, 0x00, 0x00, // payload: count=1
            0x01, 0x00, 0x01, 0x00, // payload: 0x00010001
        ];
        let data = DataContainer::from_bytes(&bytes).unwrap();
        assert_eq!(data.code, OperationCode::GetStorageIds);
        assert_eq!(data.transaction_id, 2);
        assert_eq!(data.payload.len(), 8);
        assert_eq!(
            data.payload,
            vec![0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00]
        );
    }

    #[test]
    fn data_container_roundtrip() {
        let original = DataContainer {
            code: OperationCode::GetObject,
            transaction_id: 100,
            payload: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        };
        let bytes = original.to_bytes();
        let parsed = DataContainer::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn data_container_empty_payload() {
        let data = DataContainer {
            code: OperationCode::SendObject,
            transaction_id: 5,
            payload: vec![],
        };
        let bytes = data.to_bytes();
        assert_eq!(bytes.len(), 12);
        let parsed = DataContainer::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, data);
    }

    #[test]
    fn data_container_parse_insufficient_bytes() {
        // Too small for header
        assert!(DataContainer::from_bytes(&[0x00; 11]).is_err());

        // Header says more bytes than available
        let bytes = vec![
            0x20, 0x00, 0x00, 0x00, // length = 32 (but we won't provide that many)
            0x02, 0x00, // type = Data
            0x04, 0x10, // code
            0x02, 0x00, 0x00, 0x00, // transaction_id
        ];
        assert!(DataContainer::from_bytes(&bytes).is_err());
    }

    #[test]
    fn data_container_parse_wrong_type() {
        let bytes = vec![
            0x0C, 0x00, 0x00, 0x00, // length = 12
            0x03, 0x00, // type = Response (wrong!)
            0x04, 0x10, // code
            0x02, 0x00, 0x00, 0x00, // transaction_id
        ];
        assert!(DataContainer::from_bytes(&bytes).is_err());
    }

    // =========================================================================
    // ResponseContainer tests
    // =========================================================================

    #[test]
    fn response_container_ok() {
        let bytes = vec![
            0x0C, 0x00, 0x00, 0x00, // length = 12
            0x03, 0x00, // type = Response
            0x01, 0x20, // code = OK (0x2001)
            0x01, 0x00, 0x00, 0x00, // transaction_id = 1
        ];
        let resp = ResponseContainer::from_bytes(&bytes).unwrap();
        assert_eq!(resp.code, ResponseCode::Ok);
        assert_eq!(resp.transaction_id, 1);
        assert!(resp.params.is_empty());
        assert!(resp.is_ok());
    }

    #[test]
    fn response_container_with_params() {
        let bytes = vec![
            0x18, 0x00, 0x00, 0x00, // length = 24
            0x03, 0x00, // type = Response
            0x01, 0x20, // code = OK
            0x02, 0x00, 0x00, 0x00, // transaction_id = 2
            0x01, 0x00, 0x01, 0x00, // param1 = 0x00010001
            0x00, 0x00, 0x00, 0x00, // param2 = 0
            0x05, 0x00, 0x00, 0x00, // param3 = 5
        ];
        let resp = ResponseContainer::from_bytes(&bytes).unwrap();
        assert_eq!(resp.code, ResponseCode::Ok);
        assert_eq!(resp.transaction_id, 2);
        assert_eq!(resp.params.len(), 3);
        assert_eq!(resp.params[0], 0x00010001);
        assert_eq!(resp.params[1], 0);
        assert_eq!(resp.params[2], 5);
        assert!(resp.is_ok());
    }

    #[test]
    fn response_container_error_code() {
        let bytes = vec![
            0x0C, 0x00, 0x00, 0x00, // length = 12
            0x03, 0x00, // type = Response
            0x02, 0x20, // code = GeneralError (0x2002)
            0x03, 0x00, 0x00, 0x00, // transaction_id = 3
        ];
        let resp = ResponseContainer::from_bytes(&bytes).unwrap();
        assert_eq!(resp.code, ResponseCode::GeneralError);
        assert!(!resp.is_ok());
    }

    #[test]
    fn response_container_insufficient_bytes() {
        // Too small for header
        assert!(ResponseContainer::from_bytes(&[0x00; 11]).is_err());

        // Header says more bytes than available
        let bytes = vec![
            0x18, 0x00, 0x00, 0x00, // length = 24 (but we only provide 12)
            0x03, 0x00, // type = Response
            0x01, 0x20, // code
            0x01, 0x00, 0x00, 0x00, // transaction_id
        ];
        assert!(ResponseContainer::from_bytes(&bytes).is_err());
    }

    #[test]
    fn response_container_wrong_type() {
        let bytes = vec![
            0x0C, 0x00, 0x00, 0x00, // length = 12
            0x01, 0x00, // type = Command (wrong!)
            0x01, 0x20, // code
            0x01, 0x00, 0x00, 0x00, // transaction_id
        ];
        assert!(ResponseContainer::from_bytes(&bytes).is_err());
    }

    #[test]
    fn response_container_unaligned_params() {
        // 13 bytes total = 12 header + 1 byte (not aligned to 4)
        let bytes = vec![
            0x0D, 0x00, 0x00, 0x00, // length = 13
            0x03, 0x00, // type = Response
            0x01, 0x20, // code
            0x01, 0x00, 0x00, 0x00, // transaction_id
            0xFF, // 1 extra byte (not aligned)
        ];
        assert!(ResponseContainer::from_bytes(&bytes).is_err());
    }

    // =========================================================================
    // EventContainer tests
    // =========================================================================

    #[test]
    fn event_container_object_added() {
        let bytes = vec![
            0x18, 0x00, 0x00, 0x00, // length = 24
            0x04, 0x00, // type = Event
            0x02, 0x40, // code = ObjectAdded (0x4002)
            0x00, 0x00, 0x00, 0x00, // transaction_id
            0x0A, 0x00, 0x00, 0x00, // param1 = 10
            0x00, 0x00, 0x00, 0x00, // param2
            0x00, 0x00, 0x00, 0x00, // param3
        ];
        let event = EventContainer::from_bytes(&bytes).unwrap();
        assert_eq!(event.code, EventCode::ObjectAdded);
        assert_eq!(event.transaction_id, 0);
        assert_eq!(event.params[0], 10);
        assert_eq!(event.params[1], 0);
        assert_eq!(event.params[2], 0);
    }

    #[test]
    fn event_container_store_removed() {
        let bytes = vec![
            0x18, 0x00, 0x00, 0x00, // length = 24
            0x04, 0x00, // type = Event
            0x05, 0x40, // code = StoreRemoved (0x4005)
            0x0A, 0x00, 0x00, 0x00, // transaction_id = 10
            0x01, 0x00, 0x01, 0x00, // param1 = storage id
            0x00, 0x00, 0x00, 0x00, // param2
            0x00, 0x00, 0x00, 0x00, // param3
        ];
        let event = EventContainer::from_bytes(&bytes).unwrap();
        assert_eq!(event.code, EventCode::StoreRemoved);
        assert_eq!(event.transaction_id, 10);
        assert_eq!(event.params[0], 0x00010001);
    }

    #[test]
    fn event_container_insufficient_bytes() {
        // Too small for header
        assert!(EventContainer::from_bytes(&[0x00; 11]).is_err());

        // Header ok but not enough for 3 params
        let bytes = vec![
            0x18, 0x00, 0x00, 0x00, // length = 24
            0x04, 0x00, // type = Event
            0x02, 0x40, // code
            0x00, 0x00, 0x00, 0x00, // transaction_id
                  // Missing params
        ];
        assert!(EventContainer::from_bytes(&bytes).is_err());
    }

    #[test]
    fn event_container_wrong_type() {
        let bytes = vec![
            0x18, 0x00, 0x00, 0x00, // length = 24
            0x02, 0x00, // type = Data (wrong!)
            0x02, 0x40, // code
            0x00, 0x00, 0x00, 0x00, // transaction_id
            0x00, 0x00, 0x00, 0x00, // param1
            0x00, 0x00, 0x00, 0x00, // param2
            0x00, 0x00, 0x00, 0x00, // param3
        ];
        assert!(EventContainer::from_bytes(&bytes).is_err());
    }

    #[test]
    fn event_container_wrong_length() {
        // Event must be exactly 24 bytes
        let bytes = vec![
            0x1C, 0x00, 0x00, 0x00, // length = 28 (wrong!)
            0x04, 0x00, // type = Event
            0x02, 0x40, // code
            0x00, 0x00, 0x00, 0x00, // transaction_id
            0x00, 0x00, 0x00, 0x00, // param1
            0x00, 0x00, 0x00, 0x00, // param2
            0x00, 0x00, 0x00, 0x00, // param3
            0x00, 0x00, 0x00, 0x00, // extra param (not allowed)
        ];
        assert!(EventContainer::from_bytes(&bytes).is_err());
    }

    // =========================================================================
    // Additional tests
    // =========================================================================

    #[test]
    fn command_container_get_device_info() {
        // GetDeviceInfo is special: no session needed, transaction_id = 0
        let cmd = CommandContainer {
            code: OperationCode::GetDeviceInfo,
            transaction_id: 0,
            params: vec![],
        };
        let bytes = cmd.to_bytes();
        assert_eq!(&bytes[6..8], &[0x01, 0x10]); // code = GetDeviceInfo (0x1001)
        assert_eq!(&bytes[8..12], &[0x00, 0x00, 0x00, 0x00]); // transaction_id = 0
    }

    #[test]
    fn response_container_device_busy() {
        let bytes = vec![
            0x0C, 0x00, 0x00, 0x00, // length = 12
            0x03, 0x00, // type = Response
            0x19, 0x20, // code = DeviceBusy (0x2019)
            0x05, 0x00, 0x00, 0x00, // transaction_id = 5
        ];
        let resp = ResponseContainer::from_bytes(&bytes).unwrap();
        assert_eq!(resp.code, ResponseCode::DeviceBusy);
        assert!(!resp.is_ok());
    }

    #[test]
    fn data_container_large_payload() {
        let payload: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let data = DataContainer {
            code: OperationCode::GetObject,
            transaction_id: 1,
            payload: payload.clone(),
        };
        let bytes = data.to_bytes();
        assert_eq!(bytes.len(), 12 + 1000);

        let parsed = DataContainer::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.payload, payload);
    }

    #[test]
    fn response_container_five_params() {
        let bytes = vec![
            0x20, 0x00, 0x00, 0x00, // length = 32
            0x03, 0x00, // type = Response
            0x01, 0x20, // code = OK
            0x01, 0x00, 0x00, 0x00, // transaction_id = 1
            0x01, 0x00, 0x00, 0x00, // param1 = 1
            0x02, 0x00, 0x00, 0x00, // param2 = 2
            0x03, 0x00, 0x00, 0x00, // param3 = 3
            0x04, 0x00, 0x00, 0x00, // param4 = 4
            0x05, 0x00, 0x00, 0x00, // param5 = 5
        ];
        let resp = ResponseContainer::from_bytes(&bytes).unwrap();
        assert_eq!(resp.params.len(), 5);
        assert_eq!(resp.params, vec![1, 2, 3, 4, 5]);
    }

    // =========================================================================
    // Property-based tests (proptest)
    // =========================================================================

    use proptest::prelude::*;

    // -------------------------------------------------------------------------
    // ContainerType property tests
    // -------------------------------------------------------------------------

    proptest! {
        /// Valid container types roundtrip correctly
        #[test]
        fn prop_container_type_valid_roundtrip(code in 1u16..=4u16) {
            let ct = ContainerType::from_code(code);
            prop_assert!(ct.is_some());
            prop_assert_eq!(ct.unwrap().to_code(), code);
        }

        /// Invalid container type codes return None
        #[test]
        fn prop_container_type_invalid_returns_none(code in prop::num::u16::ANY.prop_filter(
            "Must not be valid container type",
            |c| *c == 0 || *c > 4
        )) {
            prop_assert!(ContainerType::from_code(code).is_none());
        }
    }

    // -------------------------------------------------------------------------
    // DataContainer roundtrip property tests
    // -------------------------------------------------------------------------

    proptest! {
        /// DataContainer roundtrips correctly with arbitrary payloads
        #[test]
        fn prop_data_container_roundtrip(
            code in any::<u16>(),
            transaction_id in any::<u32>(),
            payload in prop::collection::vec(any::<u8>(), 0..1000)
        ) {
            let original = DataContainer {
                code: OperationCode::from_code(code),
                transaction_id,
                payload: payload.clone(),
            };
            let bytes = original.to_bytes();
            let parsed = DataContainer::from_bytes(&bytes).unwrap();

            prop_assert_eq!(parsed.code, original.code);
            prop_assert_eq!(parsed.transaction_id, original.transaction_id);
            prop_assert_eq!(parsed.payload, original.payload);
        }

        /// DataContainer length field matches actual size
        #[test]
        fn prop_data_container_length_invariant(
            code in any::<u16>(),
            transaction_id in any::<u32>(),
            payload in prop::collection::vec(any::<u8>(), 0..500)
        ) {
            let container = DataContainer {
                code: OperationCode::from_code(code),
                transaction_id,
                payload,
            };
            let bytes = container.to_bytes();

            // Length field is first 4 bytes (little-endian)
            let length = unpack_u32(&bytes[0..4]).unwrap() as usize;

            // Length should equal total bytes
            prop_assert_eq!(length, bytes.len());

            // Length should be header (12) + payload
            prop_assert_eq!(length, HEADER_SIZE + container.payload.len());
        }

        /// DataContainer type field is always Data (2)
        #[test]
        fn prop_data_container_type_field(
            code in any::<u16>(),
            transaction_id in any::<u32>(),
            payload in prop::collection::vec(any::<u8>(), 0..100)
        ) {
            let container = DataContainer {
                code: OperationCode::from_code(code),
                transaction_id,
                payload,
            };
            let bytes = container.to_bytes();

            let type_code = unpack_u16(&bytes[4..6]).unwrap();
            prop_assert_eq!(type_code, ContainerType::Data.to_code());
        }
    }

    // -------------------------------------------------------------------------
    // CommandContainer property tests
    // -------------------------------------------------------------------------

    proptest! {
        /// CommandContainer length field matches actual size
        #[test]
        fn prop_command_container_length_invariant(
            code in any::<u16>(),
            transaction_id in any::<u32>(),
            params in prop::collection::vec(any::<u32>(), 0..5)
        ) {
            let container = CommandContainer {
                code: OperationCode::from_code(code),
                transaction_id,
                params: params.clone(),
            };
            let bytes = container.to_bytes();

            // Length field is first 4 bytes (little-endian)
            let length = unpack_u32(&bytes[0..4]).unwrap() as usize;

            // Length should equal total bytes
            prop_assert_eq!(length, bytes.len());

            // Length should be header (12) + params * 4
            prop_assert_eq!(length, HEADER_SIZE + params.len() * 4);
        }

        /// CommandContainer type field is always Command (1)
        #[test]
        fn prop_command_container_type_field(
            code in any::<u16>(),
            transaction_id in any::<u32>(),
            params in prop::collection::vec(any::<u32>(), 0..5)
        ) {
            let container = CommandContainer {
                code: OperationCode::from_code(code),
                transaction_id,
                params,
            };
            let bytes = container.to_bytes();

            let type_code = unpack_u16(&bytes[4..6]).unwrap();
            prop_assert_eq!(type_code, ContainerType::Command.to_code());
        }

        /// CommandContainer preserves parameters correctly
        #[test]
        fn prop_command_container_params_preserved(
            code in any::<u16>(),
            transaction_id in any::<u32>(),
            params in prop::collection::vec(any::<u32>(), 0..5)
        ) {
            let container = CommandContainer {
                code: OperationCode::from_code(code),
                transaction_id,
                params: params.clone(),
            };
            let bytes = container.to_bytes();

            // Verify each parameter
            for (i, &expected_param) in params.iter().enumerate() {
                let offset = HEADER_SIZE + i * 4;
                let actual_param = unpack_u32(&bytes[offset..]).unwrap();
                prop_assert_eq!(actual_param, expected_param);
            }
        }
    }

    // -------------------------------------------------------------------------
    // ResponseContainer property tests
    // -------------------------------------------------------------------------

    /// Strategy for generating valid response container bytes
    fn valid_response_bytes(param_count: usize) -> impl Strategy<Value = Vec<u8>> {
        (
            any::<u16>(),                                                   // code
            any::<u32>(),                                                   // transaction_id
            prop::collection::vec(any::<u32>(), param_count..=param_count), // params
        )
            .prop_map(move |(code, transaction_id, params)| {
                let total_len = HEADER_SIZE + params.len() * 4;
                let mut bytes = Vec::with_capacity(total_len);

                // Header
                bytes.extend_from_slice(&pack_u32(total_len as u32));
                bytes.extend_from_slice(&pack_u16(ContainerType::Response.to_code()));
                bytes.extend_from_slice(&pack_u16(code));
                bytes.extend_from_slice(&pack_u32(transaction_id));

                // Parameters
                for param in &params {
                    bytes.extend_from_slice(&pack_u32(*param));
                }

                bytes
            })
    }

    proptest! {
        /// ResponseContainer parses valid bytes correctly (0 params)
        #[test]
        fn prop_response_container_parse_no_params(bytes in valid_response_bytes(0)) {
            let resp = ResponseContainer::from_bytes(&bytes).unwrap();
            prop_assert!(resp.params.is_empty());
        }

        /// ResponseContainer parses valid bytes correctly (1-5 params)
        #[test]
        fn prop_response_container_parse_with_params(param_count in 1usize..=5usize) {
            let strategy = valid_response_bytes(param_count);
            proptest!(|(bytes in strategy)| {
                let resp = ResponseContainer::from_bytes(&bytes).unwrap();
                prop_assert_eq!(resp.params.len(), param_count);
            });
        }

        /// ResponseContainer parameter count constraint (0-5 params)
        #[test]
        fn prop_response_container_param_count(param_count in 0usize..=5usize) {
            let strategy = valid_response_bytes(param_count);
            proptest!(|(bytes in strategy)| {
                let resp = ResponseContainer::from_bytes(&bytes).unwrap();
                prop_assert!(resp.params.len() <= 5);
            });
        }
    }

    // -------------------------------------------------------------------------
    // container_type() function property tests
    // -------------------------------------------------------------------------

    proptest! {
        /// container_type() correctly identifies Command containers
        #[test]
        fn prop_container_type_fn_command(
            code in any::<u16>(),
            transaction_id in any::<u32>(),
            params in prop::collection::vec(any::<u32>(), 0..5)
        ) {
            let container = CommandContainer {
                code: OperationCode::from_code(code),
                transaction_id,
                params,
            };
            let bytes = container.to_bytes();

            let ct = container_type(&bytes).unwrap();
            prop_assert_eq!(ct, ContainerType::Command);
        }

        /// container_type() correctly identifies Data containers
        #[test]
        fn prop_container_type_fn_data(
            code in any::<u16>(),
            transaction_id in any::<u32>(),
            payload in prop::collection::vec(any::<u8>(), 0..100)
        ) {
            let container = DataContainer {
                code: OperationCode::from_code(code),
                transaction_id,
                payload,
            };
            let bytes = container.to_bytes();

            let ct = container_type(&bytes).unwrap();
            prop_assert_eq!(ct, ContainerType::Data);
        }

        /// container_type() correctly identifies Response containers
        #[test]
        fn prop_container_type_fn_response(bytes in valid_response_bytes(0)) {
            let ct = container_type(&bytes).unwrap();
            prop_assert_eq!(ct, ContainerType::Response);
        }
    }

    // -------------------------------------------------------------------------
    // Edge case property tests
    // -------------------------------------------------------------------------

    proptest! {
        /// DataContainer with extra trailing bytes still parses correctly
        #[test]
        fn prop_data_container_with_extra_bytes(
            code in any::<u16>(),
            transaction_id in any::<u32>(),
            payload in prop::collection::vec(any::<u8>(), 0..100),
            extra in prop::collection::vec(any::<u8>(), 1..50)
        ) {
            let original = DataContainer {
                code: OperationCode::from_code(code),
                transaction_id,
                payload: payload.clone(),
            };
            let mut bytes = original.to_bytes();
            bytes.extend_from_slice(&extra);

            let parsed = DataContainer::from_bytes(&bytes).unwrap();
            prop_assert_eq!(parsed.payload, original.payload);
        }

        /// ResponseContainer with extra trailing bytes still parses correctly
        #[test]
        fn prop_response_container_with_extra_bytes(
            bytes in valid_response_bytes(2),
            extra in prop::collection::vec(any::<u8>(), 1..50)
        ) {
            let mut bytes_with_extra = bytes.clone();
            bytes_with_extra.extend_from_slice(&extra);

            let resp = ResponseContainer::from_bytes(&bytes_with_extra).unwrap();
            prop_assert_eq!(resp.params.len(), 2);
        }
    }
}
