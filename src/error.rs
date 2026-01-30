//! Error types for mtp-rs.

use thiserror::Error;

/// The main error type for mtp-rs operations.
#[derive(Debug, Error)]
pub enum Error {
    /// USB communication error
    #[error("USB error: {0}")]
    Usb(#[from] nusb::Error),

    /// Protocol-level error from device
    #[error("Protocol error: {code:?} during {operation:?}")]
    Protocol {
        code: crate::ptp::ResponseCode,
        operation: crate::ptp::OperationCode,
    },

    /// Invalid data received from device
    #[error("Invalid data: {message}")]
    InvalidData { message: String },

    /// I/O error
    #[error("I/O error: {0}")]
    Io(std::io::Error),

    /// Operation timed out
    #[error("Operation timed out")]
    Timeout,

    /// Device was disconnected
    #[error("Device disconnected")]
    Disconnected,

    /// Session not open
    #[error("Session not open")]
    SessionNotOpen,

    /// No device found
    #[error("No MTP device found")]
    NoDevice,

    /// Operation cancelled
    #[error("Operation cancelled")]
    Cancelled,
}

impl Error {
    /// Create an invalid data error with a message.
    pub fn invalid_data(message: impl Into<String>) -> Self {
        Error::InvalidData {
            message: message.into(),
        }
    }

    /// Check if this is a retryable error.
    ///
    /// Retryable errors are transient and the operation may succeed if retried:
    /// - `DeviceBusy`: Device is temporarily busy
    /// - `Timeout`: Operation timed out but device may still be responsive
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Error::Protocol {
                code: crate::ptp::ResponseCode::DeviceBusy,
                ..
            } | Error::Timeout
        )
    }

    /// Get the response code if this is a protocol error.
    pub fn response_code(&self) -> Option<crate::ptp::ResponseCode> {
        match self {
            Error::Protocol { code, .. } => Some(*code),
            _ => None,
        }
    }
}
