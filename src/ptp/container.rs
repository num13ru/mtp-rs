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

        // Validate length - must be at least header size and not exceed buffer
        if length < HEADER_SIZE {
            return Err(crate::Error::invalid_data(format!(
                "data container length too small: {} < header size {}",
                length, HEADER_SIZE
            )));
        }
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
    ///
    /// Events can have 0-3 parameters, so valid sizes are 12-24 bytes
    /// (header + 0-3 u32 params). Missing parameters default to 0.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::Error> {
        const MAX_EVENT_SIZE: usize = HEADER_SIZE + 12; // 24 bytes max (3 params)

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

        // Validate length: must be between 12 (header only) and 24 (header + 3 params)
        if length < HEADER_SIZE || length > MAX_EVENT_SIZE {
            return Err(crate::Error::invalid_data(format!(
                "event container invalid size: expected 12-24, got {}",
                length
            )));
        }

        // Validate parameter alignment (must be multiple of 4 bytes after header)
        let param_bytes = length - HEADER_SIZE;
        if param_bytes % 4 != 0 {
            return Err(crate::Error::invalid_data(format!(
                "event parameter bytes not aligned: {} bytes",
                param_bytes
            )));
        }

        // Validate buffer has enough data
        if buf.len() < length {
            return Err(crate::Error::invalid_data(format!(
                "event container buffer too small: need {}, have {}",
                length,
                buf.len()
            )));
        }

        // Parse parameters (0-3), defaulting missing ones to 0
        let param_count = param_bytes / 4;
        let param1 = if param_count >= 1 {
            unpack_u32(&buf[12..16])?
        } else {
            0
        };
        let param2 = if param_count >= 2 {
            unpack_u32(&buf[16..20])?
        } else {
            0
        };
        let param3 = if param_count >= 3 {
            unpack_u32(&buf[20..24])?
        } else {
            0
        };

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
    fn event_container_zero_params() {
        // Event with 0 parameters (12 bytes total)
        let bytes = vec![
            0x0C, 0x00, 0x00, 0x00, // length = 12
            0x04, 0x00, // type = Event
            0x08, 0x40, // code = DeviceInfoChanged (0x4008)
            0x00, 0x00, 0x00, 0x00, // transaction_id
        ];
        let event = EventContainer::from_bytes(&bytes).unwrap();
        assert_eq!(event.code, EventCode::DeviceInfoChanged);
        assert_eq!(event.params, [0, 0, 0]); // All default to 0
    }

    #[test]
    fn event_container_one_param() {
        // Event with 1 parameter (16 bytes total) - common on Android
        let bytes = vec![
            0x10, 0x00, 0x00, 0x00, // length = 16
            0x04, 0x00, // type = Event
            0x02, 0x40, // code = ObjectAdded (0x4002)
            0x00, 0x00, 0x00, 0x00, // transaction_id
            0x2A, 0x00, 0x00, 0x00, // param1 = 42 (ObjectHandle)
        ];
        let event = EventContainer::from_bytes(&bytes).unwrap();
        assert_eq!(event.code, EventCode::ObjectAdded);
        assert_eq!(event.params[0], 42);
        assert_eq!(event.params[1], 0); // Default
        assert_eq!(event.params[2], 0); // Default
    }

    #[test]
    fn event_container_two_params() {
        // Event with 2 parameters (20 bytes total)
        let bytes = vec![
            0x14, 0x00, 0x00, 0x00, // length = 20
            0x04, 0x00, // type = Event
            0x02, 0x40, // code = ObjectAdded
            0x05, 0x00, 0x00, 0x00, // transaction_id = 5
            0x0A, 0x00, 0x00, 0x00, // param1 = 10
            0x14, 0x00, 0x00, 0x00, // param2 = 20
        ];
        let event = EventContainer::from_bytes(&bytes).unwrap();
        assert_eq!(event.transaction_id, 5);
        assert_eq!(event.params[0], 10);
        assert_eq!(event.params[1], 20);
        assert_eq!(event.params[2], 0); // Default
    }

    #[test]
    fn event_container_length_too_large() {
        // Event with length > 24 (too many params)
        let bytes = vec![
            0x1C, 0x00, 0x00, 0x00, // length = 28 (invalid - max is 24)
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

    #[test]
    fn event_container_unaligned_length() {
        // Event with unaligned parameter bytes (14 bytes = 12 header + 2 bytes)
        let bytes = vec![
            0x0E, 0x00, 0x00, 0x00, // length = 14 (not aligned to 4)
            0x04, 0x00, // type = Event
            0x02, 0x40, // code
            0x00, 0x00, 0x00, 0x00, // transaction_id
            0x00, 0x00, // 2 extra bytes (not a full param)
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

    // =========================================================================
    // ADVERSARIAL PROPERTY-BASED TESTS
    // Goal: Find bugs by testing malformed/invalid/boundary inputs
    // =========================================================================

    // -------------------------------------------------------------------------
    // Container with wrong length field tests
    // -------------------------------------------------------------------------

    proptest! {
        /// DataContainer with length field that doesn't match actual size
        /// BUG FOUND: When length < HEADER_SIZE (12), line 168 panics:
        /// `let payload = buf[HEADER_SIZE..length].to_vec()` creates slice 12..N where N < 12
        /// This test documents the bug - currently skips values < 12 to avoid panic
        #[test]
        fn fuzz_data_container_wrong_length(
            fake_length in 12u32..1000u32, // Skip < 12 to avoid KNOWN BUG (panic)
            transaction_id: u32,
            payload in prop::collection::vec(any::<u8>(), 0..50)
        ) {
            // Build a container with a lying length field
            let mut buf = Vec::new();
            buf.extend_from_slice(&fake_length.to_le_bytes()); // Wrong length
            buf.extend_from_slice(&2u16.to_le_bytes()); // Data type
            buf.extend_from_slice(&0x1001u16.to_le_bytes()); // Some code
            buf.extend_from_slice(&transaction_id.to_le_bytes());
            buf.extend_from_slice(&payload);

            // Should handle gracefully - never panic
            let result = DataContainer::from_bytes(&buf);
            // If fake_length claims more than we have, should fail
            let actual_len = buf.len();
            if fake_length as usize > actual_len {
                prop_assert!(result.is_err());
            }
        }

        /// Test that DataContainer::from_bytes returns Err when length < HEADER_SIZE
        #[test]
        fn fuzz_data_container_length_underflow(
            fake_length in 0u32..12u32,
            transaction_id: u32,
        ) {
            let mut buf = Vec::new();
            buf.extend_from_slice(&fake_length.to_le_bytes());
            buf.extend_from_slice(&2u16.to_le_bytes()); // Data type
            buf.extend_from_slice(&0x1001u16.to_le_bytes());
            buf.extend_from_slice(&transaction_id.to_le_bytes());

            // Should return Err, not panic
            let result = DataContainer::from_bytes(&buf);
            prop_assert!(result.is_err());
        }

        /// ResponseContainer with length field claiming more data than exists
        #[test]
        fn fuzz_response_container_wrong_length(
            fake_length in 13u32..1000u32, // > HEADER_SIZE to claim params
            transaction_id: u32,
        ) {
            let mut buf = Vec::new();
            buf.extend_from_slice(&fake_length.to_le_bytes());
            buf.extend_from_slice(&3u16.to_le_bytes()); // Response type
            buf.extend_from_slice(&0x2001u16.to_le_bytes()); // OK code
            buf.extend_from_slice(&transaction_id.to_le_bytes());
            // No actual params provided

            let result = ResponseContainer::from_bytes(&buf);
            // Should fail because we claim params but don't provide them
            prop_assert!(result.is_err());
        }

        /// EventContainer with invalid length (< 12, > 24, or unaligned)
        #[test]
        fn fuzz_event_container_invalid_length(
            // Test lengths that are invalid: 0-11 (too small), 25+ (too large), or unaligned (13-15, 17-19, 21-23)
            fake_length in prop::sample::select(vec![
                0u32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, // Too small
                13, 14, 15, 17, 18, 19, 21, 22, 23,      // Unaligned
                25, 26, 28, 32, 100,                      // Too large
            ]),
            transaction_id: u32,
        ) {
            let mut buf = Vec::new();
            buf.extend_from_slice(&fake_length.to_le_bytes());
            buf.extend_from_slice(&4u16.to_le_bytes()); // Event type
            buf.extend_from_slice(&0x4002u16.to_le_bytes()); // ObjectAdded
            buf.extend_from_slice(&transaction_id.to_le_bytes());
            // Add 3 params to ensure buffer is large enough
            buf.extend_from_slice(&0u32.to_le_bytes());
            buf.extend_from_slice(&0u32.to_le_bytes());
            buf.extend_from_slice(&0u32.to_le_bytes());

            let result = EventContainer::from_bytes(&buf);
            // These lengths should all be rejected
            prop_assert!(result.is_err());
        }

        /// EventContainer with valid lengths (12, 16, 20, 24) should succeed
        #[test]
        fn fuzz_event_container_valid_length(
            valid_length in prop::sample::select(vec![12u32, 16, 20, 24]),
            transaction_id: u32,
            param1: u32,
            param2: u32,
            param3: u32,
        ) {
            let mut buf = Vec::new();
            buf.extend_from_slice(&valid_length.to_le_bytes());
            buf.extend_from_slice(&4u16.to_le_bytes()); // Event type
            buf.extend_from_slice(&0x4002u16.to_le_bytes()); // ObjectAdded
            buf.extend_from_slice(&transaction_id.to_le_bytes());
            // Add params up to what the length claims
            if valid_length >= 16 {
                buf.extend_from_slice(&param1.to_le_bytes());
            }
            if valid_length >= 20 {
                buf.extend_from_slice(&param2.to_le_bytes());
            }
            if valid_length >= 24 {
                buf.extend_from_slice(&param3.to_le_bytes());
            }

            let result = EventContainer::from_bytes(&buf);
            prop_assert!(result.is_ok());

            let event = result.unwrap();
            prop_assert_eq!(event.transaction_id, transaction_id);

            // Check params are parsed or defaulted correctly
            let param_count = (valid_length as usize - 12) / 4;
            if param_count >= 1 {
                prop_assert_eq!(event.params[0], param1);
            } else {
                prop_assert_eq!(event.params[0], 0);
            }
            if param_count >= 2 {
                prop_assert_eq!(event.params[1], param2);
            } else {
                prop_assert_eq!(event.params[1], 0);
            }
            if param_count >= 3 {
                prop_assert_eq!(event.params[2], param3);
            } else {
                prop_assert_eq!(event.params[2], 0);
            }
        }
    }

    // -------------------------------------------------------------------------
    // Container with invalid type code tests
    // -------------------------------------------------------------------------

    proptest! {
        /// Container with invalid type code (0, 5+)
        #[test]
        fn fuzz_container_invalid_type(
            length in 12u32..100u32,
            invalid_type in prop::sample::select(vec![0u16, 5, 6, 100, 0xFFFF]),
            code: u16,
            transaction_id: u32,
        ) {
            let mut buf = Vec::new();
            buf.extend_from_slice(&length.to_le_bytes());
            buf.extend_from_slice(&invalid_type.to_le_bytes());
            buf.extend_from_slice(&code.to_le_bytes());
            buf.extend_from_slice(&transaction_id.to_le_bytes());

            let result = container_type(&buf);
            // Should return error for invalid types
            prop_assert!(result.is_err());
        }

        /// DataContainer with wrong container type in header
        #[test]
        fn fuzz_data_container_wrong_type(
            transaction_id: u32,
            payload in prop::collection::vec(any::<u8>(), 0..20),
            wrong_type in prop::sample::select(vec![1u16, 3, 4]), // Command, Response, Event
        ) {
            let total_len = 12 + payload.len();
            let mut buf = Vec::new();
            buf.extend_from_slice(&(total_len as u32).to_le_bytes());
            buf.extend_from_slice(&wrong_type.to_le_bytes()); // Wrong type!
            buf.extend_from_slice(&0x1001u16.to_le_bytes());
            buf.extend_from_slice(&transaction_id.to_le_bytes());
            buf.extend_from_slice(&payload);

            let result = DataContainer::from_bytes(&buf);
            prop_assert!(result.is_err());
        }

        /// ResponseContainer with wrong container type in header
        #[test]
        fn fuzz_response_container_wrong_type(
            transaction_id: u32,
            wrong_type in prop::sample::select(vec![1u16, 2, 4]), // Command, Data, Event
        ) {
            let mut buf = Vec::new();
            buf.extend_from_slice(&12u32.to_le_bytes());
            buf.extend_from_slice(&wrong_type.to_le_bytes()); // Wrong type!
            buf.extend_from_slice(&0x2001u16.to_le_bytes());
            buf.extend_from_slice(&transaction_id.to_le_bytes());

            let result = ResponseContainer::from_bytes(&buf);
            prop_assert!(result.is_err());
        }

        /// EventContainer with wrong container type in header
        #[test]
        fn fuzz_event_container_wrong_type(
            transaction_id: u32,
            wrong_type in prop::sample::select(vec![1u16, 2, 3]), // Command, Data, Response
        ) {
            let mut buf = Vec::new();
            buf.extend_from_slice(&24u32.to_le_bytes());
            buf.extend_from_slice(&wrong_type.to_le_bytes()); // Wrong type!
            buf.extend_from_slice(&0x4002u16.to_le_bytes());
            buf.extend_from_slice(&transaction_id.to_le_bytes());
            buf.extend_from_slice(&0u32.to_le_bytes());
            buf.extend_from_slice(&0u32.to_le_bytes());
            buf.extend_from_slice(&0u32.to_le_bytes());

            let result = EventContainer::from_bytes(&buf);
            prop_assert!(result.is_err());
        }
    }

    // -------------------------------------------------------------------------
    // Response container unaligned parameters test
    // -------------------------------------------------------------------------

    proptest! {
        /// ResponseContainer with unaligned parameter bytes (not multiple of 4)
        #[test]
        fn fuzz_response_container_unaligned(
            code: u16,
            transaction_id: u32,
            extra_bytes in prop::collection::vec(any::<u8>(), 1..4), // 1-3 bytes, not aligned
        ) {
            // Only test lengths 1, 2, 3 (not 0 or 4)
            prop_assume!(!extra_bytes.is_empty() && extra_bytes.len() < 4);

            let length = 12 + extra_bytes.len() as u32;
            let mut buf = Vec::new();
            buf.extend_from_slice(&length.to_le_bytes());
            buf.extend_from_slice(&3u16.to_le_bytes()); // Response type
            buf.extend_from_slice(&code.to_le_bytes());
            buf.extend_from_slice(&transaction_id.to_le_bytes());
            buf.extend_from_slice(&extra_bytes);

            // Should reject unaligned parameter bytes
            let result = ResponseContainer::from_bytes(&buf);
            prop_assert!(result.is_err());
        }
    }

    // -------------------------------------------------------------------------
    // Completely random garbage tests - should never panic
    // -------------------------------------------------------------------------

    proptest! {
        /// Random bytes as container_type() input - should never panic
        #[test]
        fn fuzz_container_type_garbage(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
            let _ = container_type(&bytes);
        }

        /// Random bytes as DataContainer - should never panic
        #[test]
        fn fuzz_data_container_garbage(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
            let _ = DataContainer::from_bytes(&bytes);
        }

        /// Random bytes as ResponseContainer - should never panic
        #[test]
        fn fuzz_response_container_garbage(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
            let _ = ResponseContainer::from_bytes(&bytes);
        }

        /// Random bytes as EventContainer - should never panic
        #[test]
        fn fuzz_event_container_garbage(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
            let _ = EventContainer::from_bytes(&bytes);
        }
    }

    // -------------------------------------------------------------------------
    // Length field edge cases / overflow potential
    // -------------------------------------------------------------------------

    // Note: fuzz_data_container_tiny_length removed - covered by
    // fuzz_data_container_length_underflow_bug which documents the panic bug

    proptest! {
        /// Container with u32::MAX length
        #[test]
        fn fuzz_container_max_length(transaction_id: u32) {
            let mut buf = Vec::new();
            buf.extend_from_slice(&u32::MAX.to_le_bytes());
            buf.extend_from_slice(&2u16.to_le_bytes());
            buf.extend_from_slice(&0x1001u16.to_le_bytes());
            buf.extend_from_slice(&transaction_id.to_le_bytes());

            // Claims to be 4GB, but we only have 12 bytes
            let result = DataContainer::from_bytes(&buf);
            prop_assert!(result.is_err());
        }
    }

    // -------------------------------------------------------------------------
    // Boundary tests for header size
    // -------------------------------------------------------------------------

    #[test]
    fn container_type_exactly_11_bytes() {
        let buf = [0u8; 11];
        assert!(container_type(&buf).is_err());
    }

    #[test]
    fn container_type_exactly_12_bytes_valid() {
        let mut buf = [0u8; 12];
        buf[4] = 1; // Type = Command
        assert!(container_type(&buf).is_ok());
    }

    #[test]
    fn data_container_exactly_12_bytes_empty_payload() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&12u32.to_le_bytes()); // length = 12
        buf.extend_from_slice(&2u16.to_le_bytes()); // Data type
        buf.extend_from_slice(&0x1001u16.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());

        let result = DataContainer::from_bytes(&buf).unwrap();
        assert!(result.payload.is_empty());
    }

    #[test]
    fn response_container_exactly_12_bytes_no_params() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&12u32.to_le_bytes()); // length = 12
        buf.extend_from_slice(&3u16.to_le_bytes()); // Response type
        buf.extend_from_slice(&0x2001u16.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());

        let result = ResponseContainer::from_bytes(&buf).unwrap();
        assert!(result.params.is_empty());
    }
}
