//! Error types for mtp-rs.

use crate::ptp::ObjectHandle;
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
        /// The response code returned by the device.
        code: crate::ptp::ResponseCode,
        /// The operation that triggered the error.
        operation: crate::ptp::OperationCode,
    },

    /// Invalid data received from device
    #[error("Invalid data: {message}")]
    InvalidData {
        /// Description of what was invalid.
        message: String,
    },

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

/// Error from an upload, carrying the handle of the object the device created
/// during `SendObjectInfo` before the data phase failed.
///
/// PTP uploads are two-phase: `SendObjectInfo` creates the object on the device
/// (returning a handle), then `SendObject` streams the bytes. If the data phase
/// fails or is cancelled, the device is left holding a partial (often empty or
/// truncated) object. This error surfaces that handle so the caller owns the
/// cleanup-or-resume decision, rather than the library guessing.
///
/// The library does **not** auto-delete the partial object: deleting it would
/// issue hidden USB I/O to a possibly-disconnected device, the leave-vs-delete
/// behavior is device-dependent, and PTP's two-phase model is designed so a
/// failed `SendObject` can be retried against the same handle (resume).
///
/// [`From<UploadError> for Error`] keeps `?` ergonomic for callers working in an
/// [`enum@Error`] context; they drop the [`partial`](Self::partial) handle unless
/// they match on `UploadError` explicitly.
#[derive(Debug, Error)]
#[error("{source}")]
pub struct UploadError {
    /// The underlying failure (I/O, protocol, cancellation, timeout, …).
    #[source]
    pub source: Error,
    /// The handle of the partially-written object the device may still hold.
    ///
    /// `Some` iff `SendObjectInfo` succeeded but the data phase did not complete
    /// (genuine error OR cancellation). The object may be empty or truncated. The
    /// caller decides: delete it (for example, [`Storage::delete`]) to discard the
    /// corrupt artifact, or retry the data phase to resume.
    ///
    /// `None` iff no object was created (for example, `SendObjectInfo` itself
    /// failed because the storage is read-only or the parent is invalid).
    ///
    /// [`Storage::delete`]: crate::mtp::Storage::delete
    pub partial: Option<ObjectHandle>,
}

impl From<UploadError> for Error {
    fn from(e: UploadError) -> Self {
        e.source
    }
}

impl Error {
    /// Create an invalid data error with a message.
    #[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn response_code(&self) -> Option<crate::ptp::ResponseCode> {
        match self {
            Error::Protocol { code, .. } => Some(*code),
            _ => None,
        }
    }

    /// Check if this error indicates another process has exclusive access to the device.
    ///
    /// This typically happens on macOS when `ptpcamerad` or another application
    /// has already claimed the USB interface. Applications can use this to provide
    /// platform-specific guidance to users.
    ///
    /// # Example
    ///
    /// ```ignore
    /// match device.open().await {
    ///     Err(e) if e.is_exclusive_access() => {
    ///         // On macOS, likely ptpcamerad interference
    ///         // App can query IORegistry for UsbExclusiveOwner to get details
    ///         show_exclusive_access_help();
    ///     }
    ///     Err(e) => handle_other_error(e),
    ///     Ok(dev) => use_device(dev),
    /// }
    /// ```
    #[must_use]
    pub fn is_exclusive_access(&self) -> bool {
        match self {
            Error::Usb(io_err) => {
                let msg = io_err.to_string().to_lowercase();
                // macOS: "could not be opened for exclusive access"
                // Linux: typically EBUSY, but message varies
                // Windows: "access denied" or similar
                msg.contains("exclusive access")
                    || msg.contains("device or resource busy")
                    || (msg.contains("access") && msg.contains("denied"))
            }
            Error::Io(io_err) => {
                let msg = io_err.to_string().to_lowercase();
                msg.contains("exclusive access")
                    || msg.contains("device or resource busy")
                    || (msg.contains("access") && msg.contains("denied"))
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error as IoError, ErrorKind};

    #[test]
    fn test_is_exclusive_access_macos_message() {
        // macOS nusb error message (tested via Io variant; same logic as Usb variant)
        let io_err = IoError::other("could not be opened for exclusive access");
        let err = Error::Io(io_err);
        assert!(err.is_exclusive_access());
    }

    #[test]
    fn test_is_exclusive_access_linux_busy() {
        // Linux EBUSY style message (tested via Io variant; same logic as Usb variant)
        let io_err = IoError::other("Device or resource busy");
        let err = Error::Io(io_err);
        assert!(err.is_exclusive_access());
    }

    #[test]
    fn test_is_exclusive_access_windows_denied() {
        // Windows access denied style message (tested via Io variant; same logic as Usb variant)
        let io_err = IoError::new(ErrorKind::PermissionDenied, "Access is denied");
        let err = Error::Io(io_err);
        assert!(err.is_exclusive_access());
    }

    #[test]
    fn test_is_exclusive_access_io_error() {
        // Also works for Io variant
        let io_err = IoError::other("could not be opened for exclusive access");
        let err = Error::Io(io_err);
        assert!(err.is_exclusive_access());
    }

    #[test]
    fn test_is_exclusive_access_false_for_other_errors() {
        assert!(!Error::Timeout.is_exclusive_access());
        assert!(!Error::Disconnected.is_exclusive_access());
        assert!(!Error::NoDevice.is_exclusive_access());
        assert!(!Error::invalid_data("some error").is_exclusive_access());

        let io_err = IoError::new(ErrorKind::NotFound, "device not found");
        assert!(!Error::Io(io_err).is_exclusive_access());
    }
}
