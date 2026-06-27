//! Backend-neutral error type for the high-level [`crate::mtp`] API.
//!
//! [`enum@Error`] is deliberately free of any single backend's vocabulary. The PTP-over-USB backend
//! maps device response codes and USB faults into it; the Windows WPD backend maps `HRESULT`s into
//! the same variants. Consumers switch on these neutral cases instead of on a backend's protocol
//! codes. Low-level / camera users who need the raw PTP response codes use the [`crate::ptp`] layer,
//! which keeps its detailed error type.

use crate::mtp::ObjectHandle;
use crate::ptp::ResponseCode;
use thiserror::Error;

/// The error type for high-level [`crate::mtp`] operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// The object or storage was not found (or never existed).
    #[error("not found")]
    NotFound,

    /// A previously-valid handle is no longer valid because the device re-keyed it.
    ///
    /// Notably Android's MediaProvider re-keys object IDs across a media rescan, so a cached handle
    /// can be silently invalidated. The fix is to re-list the parent, re-resolve, and retry once —
    /// not to treat it as a hard not-found. See `AGENTS.md`.
    #[error("stale object handle (device re-keyed it; re-list and retry)")]
    StaleHandle,

    /// The operation was refused: read-only storage, write-protected object, or denied access.
    #[error("access denied")]
    AccessDenied,

    /// Another process holds the device exclusively (e.g. `ptpcamerad` on macOS, or a busy claim).
    ///
    /// Use this to guide users to close the conflicting app.
    #[error("device is held exclusively by another process")]
    ExclusiveAccess,

    /// The device does not support this operation.
    #[error("operation not supported by this device")]
    Unsupported,

    /// The device is temporarily busy; retrying may succeed.
    #[error("device busy")]
    Busy,

    /// The target storage is full.
    #[error("storage full")]
    StorageFull,

    /// The operation was cancelled (via a `CancelToken` or a stream cancel/drop).
    #[error("operation cancelled")]
    Cancelled,

    /// The device was disconnected or stopped responding.
    #[error("device disconnected")]
    Disconnected,

    /// The operation timed out.
    #[error("operation timed out")]
    Timeout,

    /// No matching device was found.
    #[error("no device found")]
    NoDevice,

    /// Data received from the device couldn't be interpreted.
    #[error("invalid data: {message}")]
    InvalidData {
        /// What was invalid.
        message: String,
    },

    /// An I/O error not covered by a more specific variant.
    #[error("I/O error: {message}")]
    Io {
        /// The underlying message.
        message: String,
    },

    /// A backend error without a more specific neutral mapping.
    ///
    /// `detail` carries backend-specific text (a PTP response code, an `HRESULT`) for diagnostics
    /// only — don't pattern-match on its contents.
    #[error("device error: {detail}")]
    Other {
        /// Backend-specific diagnostic text.
        detail: String,
    },
}

impl Error {
    /// Create an [`Error::InvalidData`] with a message.
    #[must_use]
    pub fn invalid_data(message: impl Into<String>) -> Self {
        Error::InvalidData {
            message: message.into(),
        }
    }

    /// Whether retrying the operation might succeed (transient failures).
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(self, Error::Busy | Error::Timeout)
    }

    /// Whether another process holds the device exclusively.
    ///
    /// Applications can use this to guide users to close the conflicting app (for example, query
    /// IORegistry for `UsbExclusiveOwner` on macOS).
    #[must_use]
    pub fn is_exclusive_access(&self) -> bool {
        matches!(self, Error::ExclusiveAccess)
    }

    /// Whether this is the Android "re-key" case where re-listing the parent and retrying once is
    /// the correct recovery (rather than treating it as not-found).
    #[must_use]
    pub fn is_stale_handle(&self) -> bool {
        matches!(self, Error::StaleHandle)
    }
}

impl From<crate::error::PtpError> for Error {
    fn from(e: crate::error::PtpError) -> Self {
        use crate::error::PtpError as Low;
        let exclusive = e.is_exclusive_access();
        match e {
            Low::Protocol { code, .. } => map_response_code(code),
            Low::Usb(_) if exclusive => Error::ExclusiveAccess,
            Low::Usb(io) => {
                let msg = io.to_string().to_lowercase();
                if msg.contains("no such device") || msg.contains("disconnect") {
                    Error::Disconnected
                } else {
                    Error::Io {
                        message: io.to_string(),
                    }
                }
            }
            Low::Io(_) if exclusive => Error::ExclusiveAccess,
            Low::Io(io) => Error::Io {
                message: io.to_string(),
            },
            Low::InvalidData { message } => Error::InvalidData { message },
            Low::Timeout => Error::Timeout,
            Low::Disconnected => Error::Disconnected,
            Low::SessionNotOpen => Error::Disconnected,
            Low::NoDevice => Error::NoDevice,
            Low::Cancelled => Error::Cancelled,
        }
    }
}

/// Map a PTP response code to a neutral [`enum@Error`].
fn map_response_code(code: ResponseCode) -> Error {
    match code {
        // A previously-valid handle/parent going invalid is the Android re-key case (recoverable),
        // not a hard not-found. Callers re-list the parent and retry once.
        ResponseCode::InvalidObjectHandle | ResponseCode::InvalidParentObject => Error::StaleHandle,
        ResponseCode::InvalidStorageId => Error::NotFound,
        ResponseCode::StoreReadOnly
        | ResponseCode::ObjectWriteProtected
        | ResponseCode::AccessDenied => Error::AccessDenied,
        ResponseCode::StoreFull | ResponseCode::ObjectTooLarge => Error::StorageFull,
        ResponseCode::DeviceBusy => Error::Busy,
        ResponseCode::OperationNotSupported | ResponseCode::ParameterNotSupported => {
            Error::Unsupported
        }
        ResponseCode::TransactionCancelled => Error::Cancelled,
        ResponseCode::SessionNotOpen => Error::Disconnected,
        other => Error::Other {
            detail: format!("{other:?}"),
        },
    }
}

/// Error from a high-level upload, carrying the handle of the object the device created during the
/// first phase before the data phase failed.
///
/// Uploads are two-phase: the object is created (returning a handle), then the bytes are streamed.
/// If the data phase fails or is cancelled, the device may keep a partial (empty or truncated)
/// object. This surfaces that handle so the caller owns the cleanup-or-resume decision; the library
/// never auto-deletes it. (Backends differ: the WPD backend commits atomically, so `partial` is
/// `None` there.)
///
/// [`From<UploadError> for Error`] keeps `?` ergonomic; callers drop [`partial`](Self::partial)
/// unless they match on `UploadError` explicitly.
#[derive(Debug, Error)]
#[error("{source}")]
pub struct UploadError {
    /// The underlying failure.
    #[source]
    pub source: Error,
    /// The handle of the partially-written object the device may still hold, if any.
    pub partial: Option<ObjectHandle>,
}

impl From<UploadError> for Error {
    fn from(e: UploadError) -> Self {
        e.source
    }
}

impl From<crate::error::PtpUploadError> for UploadError {
    fn from(e: crate::error::PtpUploadError) -> Self {
        UploadError {
            source: e.source.into(),
            partial: e.partial.map(Into::into),
        }
    }
}
