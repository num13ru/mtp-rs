//! MTP/PTP protocol operation, response, event, and format codes.
//!
//! This module defines the standard codes used in MTP/PTP communication:
//! - [`OperationCode`]: Commands sent to the device
//! - [`ResponseCode`]: Status codes returned by the device
//! - [`EventCode`]: Asynchronous events from the device
//! - [`ObjectFormatCode`]: File format identifiers

/// PTP operation codes (commands sent to device).
///
/// These codes identify the operation being requested in a PTP command container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum OperationCode {
    /// Get device information (capabilities, manufacturer, etc.).
    GetDeviceInfo = 0x1001,
    /// Open a session with the device.
    OpenSession = 0x1002,
    /// Close the current session.
    CloseSession = 0x1003,
    /// Get list of storage IDs.
    GetStorageIds = 0x1004,
    /// Get information about a storage.
    GetStorageInfo = 0x1005,
    /// Get the number of objects in a storage/folder.
    GetNumObjects = 0x1006,
    /// Get list of object handles.
    GetObjectHandles = 0x1007,
    /// Get information about an object.
    GetObjectInfo = 0x1008,
    /// Download an object's data.
    GetObject = 0x1009,
    /// Get thumbnail for an object.
    GetThumb = 0x100A,
    /// Delete an object.
    DeleteObject = 0x100B,
    /// Send object metadata (before sending object data).
    SendObjectInfo = 0x100C,
    /// Send object data (after SendObjectInfo).
    SendObject = 0x100D,
    /// Move an object to a different location.
    MoveObject = 0x1019,
    /// Copy an object.
    CopyObject = 0x101A,
    /// Get partial object data (range request).
    GetPartialObject = 0x101B,
    /// Unknown or vendor-specific operation code.
    Unknown(u16),
}

impl OperationCode {
    /// Convert a raw u16 code to an OperationCode.
    pub fn from_code(code: u16) -> Self {
        match code {
            0x1001 => OperationCode::GetDeviceInfo,
            0x1002 => OperationCode::OpenSession,
            0x1003 => OperationCode::CloseSession,
            0x1004 => OperationCode::GetStorageIds,
            0x1005 => OperationCode::GetStorageInfo,
            0x1006 => OperationCode::GetNumObjects,
            0x1007 => OperationCode::GetObjectHandles,
            0x1008 => OperationCode::GetObjectInfo,
            0x1009 => OperationCode::GetObject,
            0x100A => OperationCode::GetThumb,
            0x100B => OperationCode::DeleteObject,
            0x100C => OperationCode::SendObjectInfo,
            0x100D => OperationCode::SendObject,
            0x1019 => OperationCode::MoveObject,
            0x101A => OperationCode::CopyObject,
            0x101B => OperationCode::GetPartialObject,
            _ => OperationCode::Unknown(code),
        }
    }

    /// Convert an OperationCode to its raw u16 value.
    pub fn to_code(self) -> u16 {
        match self {
            OperationCode::GetDeviceInfo => 0x1001,
            OperationCode::OpenSession => 0x1002,
            OperationCode::CloseSession => 0x1003,
            OperationCode::GetStorageIds => 0x1004,
            OperationCode::GetStorageInfo => 0x1005,
            OperationCode::GetNumObjects => 0x1006,
            OperationCode::GetObjectHandles => 0x1007,
            OperationCode::GetObjectInfo => 0x1008,
            OperationCode::GetObject => 0x1009,
            OperationCode::GetThumb => 0x100A,
            OperationCode::DeleteObject => 0x100B,
            OperationCode::SendObjectInfo => 0x100C,
            OperationCode::SendObject => 0x100D,
            OperationCode::MoveObject => 0x1019,
            OperationCode::CopyObject => 0x101A,
            OperationCode::GetPartialObject => 0x101B,
            OperationCode::Unknown(code) => code,
        }
    }
}

/// PTP response codes (status returned by device).
///
/// These codes indicate the result of an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ResponseCode {
    /// Operation completed successfully.
    Ok = 0x2001,
    /// General unspecified error.
    GeneralError = 0x2002,
    /// Session is not open.
    SessionNotOpen = 0x2003,
    /// Invalid transaction ID.
    InvalidTransactionId = 0x2004,
    /// Operation is not supported.
    OperationNotSupported = 0x2005,
    /// Parameter is not supported.
    ParameterNotSupported = 0x2006,
    /// Transfer was incomplete.
    IncompleteTransfer = 0x2007,
    /// Invalid storage ID.
    InvalidStorageId = 0x2008,
    /// Invalid object handle.
    InvalidObjectHandle = 0x2009,
    /// Storage is full.
    StoreFull = 0x200C,
    /// Object is write-protected.
    ObjectWriteProtected = 0x200D,
    /// Storage is read-only.
    StoreReadOnly = 0x200E,
    /// Access denied.
    AccessDenied = 0x200F,
    /// Object has no thumbnail.
    NoThumbnailPresent = 0x2010,
    /// Device is busy.
    DeviceBusy = 0x2019,
    /// Invalid parent object.
    InvalidParentObject = 0x201A,
    /// Invalid parameter value.
    InvalidParameter = 0x201D,
    /// Session is already open.
    SessionAlreadyOpen = 0x201E,
    /// Transaction was cancelled.
    TransactionCancelled = 0x201F,
    /// Object is too large for the storage.
    ObjectTooLarge = 0xA809,
    /// Unknown or vendor-specific response code.
    Unknown(u16),
}

impl ResponseCode {
    /// Convert a raw u16 code to a ResponseCode.
    pub fn from_code(code: u16) -> Self {
        match code {
            0x2001 => ResponseCode::Ok,
            0x2002 => ResponseCode::GeneralError,
            0x2003 => ResponseCode::SessionNotOpen,
            0x2004 => ResponseCode::InvalidTransactionId,
            0x2005 => ResponseCode::OperationNotSupported,
            0x2006 => ResponseCode::ParameterNotSupported,
            0x2007 => ResponseCode::IncompleteTransfer,
            0x2008 => ResponseCode::InvalidStorageId,
            0x2009 => ResponseCode::InvalidObjectHandle,
            0x200C => ResponseCode::StoreFull,
            0x200D => ResponseCode::ObjectWriteProtected,
            0x200E => ResponseCode::StoreReadOnly,
            0x200F => ResponseCode::AccessDenied,
            0x2010 => ResponseCode::NoThumbnailPresent,
            0x2019 => ResponseCode::DeviceBusy,
            0x201A => ResponseCode::InvalidParentObject,
            0x201D => ResponseCode::InvalidParameter,
            0x201E => ResponseCode::SessionAlreadyOpen,
            0x201F => ResponseCode::TransactionCancelled,
            0xA809 => ResponseCode::ObjectTooLarge,
            _ => ResponseCode::Unknown(code),
        }
    }

    /// Convert a ResponseCode to its raw u16 value.
    pub fn to_code(self) -> u16 {
        match self {
            ResponseCode::Ok => 0x2001,
            ResponseCode::GeneralError => 0x2002,
            ResponseCode::SessionNotOpen => 0x2003,
            ResponseCode::InvalidTransactionId => 0x2004,
            ResponseCode::OperationNotSupported => 0x2005,
            ResponseCode::ParameterNotSupported => 0x2006,
            ResponseCode::IncompleteTransfer => 0x2007,
            ResponseCode::InvalidStorageId => 0x2008,
            ResponseCode::InvalidObjectHandle => 0x2009,
            ResponseCode::StoreFull => 0x200C,
            ResponseCode::ObjectWriteProtected => 0x200D,
            ResponseCode::StoreReadOnly => 0x200E,
            ResponseCode::AccessDenied => 0x200F,
            ResponseCode::NoThumbnailPresent => 0x2010,
            ResponseCode::DeviceBusy => 0x2019,
            ResponseCode::InvalidParentObject => 0x201A,
            ResponseCode::InvalidParameter => 0x201D,
            ResponseCode::SessionAlreadyOpen => 0x201E,
            ResponseCode::TransactionCancelled => 0x201F,
            ResponseCode::ObjectTooLarge => 0xA809,
            ResponseCode::Unknown(code) => code,
        }
    }
}

/// PTP event codes (asynchronous notifications from device).
///
/// These codes identify events that the device sends asynchronously.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum EventCode {
    /// A new object was added.
    ObjectAdded = 0x4002,
    /// An object was removed.
    ObjectRemoved = 0x4003,
    /// A new storage was added.
    StoreAdded = 0x4004,
    /// A storage was removed.
    StoreRemoved = 0x4005,
    /// A device property changed.
    DevicePropChanged = 0x4006,
    /// Object information changed.
    ObjectInfoChanged = 0x4007,
    /// Device information changed.
    DeviceInfoChanged = 0x4008,
    /// Storage information changed.
    StorageInfoChanged = 0x400C,
    /// Unknown or vendor-specific event code.
    Unknown(u16),
}

impl EventCode {
    /// Convert a raw u16 code to an EventCode.
    pub fn from_code(code: u16) -> Self {
        match code {
            0x4002 => EventCode::ObjectAdded,
            0x4003 => EventCode::ObjectRemoved,
            0x4004 => EventCode::StoreAdded,
            0x4005 => EventCode::StoreRemoved,
            0x4006 => EventCode::DevicePropChanged,
            0x4007 => EventCode::ObjectInfoChanged,
            0x4008 => EventCode::DeviceInfoChanged,
            0x400C => EventCode::StorageInfoChanged,
            _ => EventCode::Unknown(code),
        }
    }

    /// Convert an EventCode to its raw u16 value.
    pub fn to_code(self) -> u16 {
        match self {
            EventCode::ObjectAdded => 0x4002,
            EventCode::ObjectRemoved => 0x4003,
            EventCode::StoreAdded => 0x4004,
            EventCode::StoreRemoved => 0x4005,
            EventCode::DevicePropChanged => 0x4006,
            EventCode::ObjectInfoChanged => 0x4007,
            EventCode::DeviceInfoChanged => 0x4008,
            EventCode::StorageInfoChanged => 0x400C,
            EventCode::Unknown(code) => code,
        }
    }
}

/// PTP/MTP object format codes.
///
/// These codes identify the format/type of objects stored on the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ObjectFormatCode {
    /// Undefined/unknown format.
    Undefined = 0x3000,
    /// Association (folder/directory).
    Association = 0x3001,
    /// Script file.
    Script = 0x3002,
    /// Executable file.
    Executable = 0x3003,
    /// Plain text file.
    Text = 0x3004,
    /// HTML file.
    Html = 0x3005,
    /// DPOF (Digital Print Order Format).
    Dpof = 0x3006,
    /// AIFF audio.
    Aiff = 0x3007,
    /// WAV audio.
    Wav = 0x3008,
    /// MP3 audio.
    Mp3 = 0x3009,
    /// AVI video.
    Avi = 0x300A,
    /// MPEG video.
    Mpeg = 0x300B,
    /// ASF (Advanced Systems Format).
    Asf = 0x300C,
    /// JPEG image.
    Jpeg = 0x3801,
    /// TIFF image.
    Tiff = 0x3804,
    /// GIF image.
    Gif = 0x3807,
    /// BMP image.
    Bmp = 0x3808,
    /// PICT image.
    Pict = 0x380A,
    /// PNG image.
    Png = 0x380B,
    /// WMA audio.
    WmaAudio = 0xB901,
    /// OGG audio.
    OggAudio = 0xB902,
    /// AAC audio.
    AacAudio = 0xB903,
    /// FLAC audio.
    FlacAudio = 0xB906,
    /// WMV video.
    WmvVideo = 0xB981,
    /// MP4 container.
    Mp4Container = 0xB982,
    /// M4A audio.
    M4aAudio = 0xB984,
    /// Unknown or vendor-specific format code.
    Unknown(u16),
}

impl ObjectFormatCode {
    /// Convert a raw u16 code to an ObjectFormatCode.
    pub fn from_code(code: u16) -> Self {
        match code {
            0x3000 => ObjectFormatCode::Undefined,
            0x3001 => ObjectFormatCode::Association,
            0x3002 => ObjectFormatCode::Script,
            0x3003 => ObjectFormatCode::Executable,
            0x3004 => ObjectFormatCode::Text,
            0x3005 => ObjectFormatCode::Html,
            0x3006 => ObjectFormatCode::Dpof,
            0x3007 => ObjectFormatCode::Aiff,
            0x3008 => ObjectFormatCode::Wav,
            0x3009 => ObjectFormatCode::Mp3,
            0x300A => ObjectFormatCode::Avi,
            0x300B => ObjectFormatCode::Mpeg,
            0x300C => ObjectFormatCode::Asf,
            0x3801 => ObjectFormatCode::Jpeg,
            0x3804 => ObjectFormatCode::Tiff,
            0x3807 => ObjectFormatCode::Gif,
            0x3808 => ObjectFormatCode::Bmp,
            0x380A => ObjectFormatCode::Pict,
            0x380B => ObjectFormatCode::Png,
            0xB901 => ObjectFormatCode::WmaAudio,
            0xB902 => ObjectFormatCode::OggAudio,
            0xB903 => ObjectFormatCode::AacAudio,
            0xB906 => ObjectFormatCode::FlacAudio,
            0xB981 => ObjectFormatCode::WmvVideo,
            0xB982 => ObjectFormatCode::Mp4Container,
            0xB984 => ObjectFormatCode::M4aAudio,
            _ => ObjectFormatCode::Unknown(code),
        }
    }

    /// Convert an ObjectFormatCode to its raw u16 value.
    pub fn to_code(self) -> u16 {
        match self {
            ObjectFormatCode::Undefined => 0x3000,
            ObjectFormatCode::Association => 0x3001,
            ObjectFormatCode::Script => 0x3002,
            ObjectFormatCode::Executable => 0x3003,
            ObjectFormatCode::Text => 0x3004,
            ObjectFormatCode::Html => 0x3005,
            ObjectFormatCode::Dpof => 0x3006,
            ObjectFormatCode::Aiff => 0x3007,
            ObjectFormatCode::Wav => 0x3008,
            ObjectFormatCode::Mp3 => 0x3009,
            ObjectFormatCode::Avi => 0x300A,
            ObjectFormatCode::Mpeg => 0x300B,
            ObjectFormatCode::Asf => 0x300C,
            ObjectFormatCode::Jpeg => 0x3801,
            ObjectFormatCode::Tiff => 0x3804,
            ObjectFormatCode::Gif => 0x3807,
            ObjectFormatCode::Bmp => 0x3808,
            ObjectFormatCode::Pict => 0x380A,
            ObjectFormatCode::Png => 0x380B,
            ObjectFormatCode::WmaAudio => 0xB901,
            ObjectFormatCode::OggAudio => 0xB902,
            ObjectFormatCode::AacAudio => 0xB903,
            ObjectFormatCode::FlacAudio => 0xB906,
            ObjectFormatCode::WmvVideo => 0xB981,
            ObjectFormatCode::Mp4Container => 0xB982,
            ObjectFormatCode::M4aAudio => 0xB984,
            ObjectFormatCode::Unknown(code) => code,
        }
    }

    /// Detect object format from file extension (case insensitive).
    ///
    /// Returns `Undefined` for unrecognized extensions.
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            // Text and documents
            "txt" => ObjectFormatCode::Text,
            "html" | "htm" => ObjectFormatCode::Html,
            "dpof" => ObjectFormatCode::Dpof,

            // Audio formats
            "aiff" | "aif" => ObjectFormatCode::Aiff,
            "wav" => ObjectFormatCode::Wav,
            "mp3" => ObjectFormatCode::Mp3,
            "wma" => ObjectFormatCode::WmaAudio,
            "ogg" | "oga" => ObjectFormatCode::OggAudio,
            "aac" => ObjectFormatCode::AacAudio,
            "flac" => ObjectFormatCode::FlacAudio,
            "m4a" => ObjectFormatCode::M4aAudio,

            // Video formats
            "avi" => ObjectFormatCode::Avi,
            "mpg" | "mpeg" => ObjectFormatCode::Mpeg,
            "asf" => ObjectFormatCode::Asf,
            "wmv" => ObjectFormatCode::WmvVideo,
            "mp4" | "m4v" => ObjectFormatCode::Mp4Container,

            // Image formats
            "jpg" | "jpeg" => ObjectFormatCode::Jpeg,
            "tif" | "tiff" => ObjectFormatCode::Tiff,
            "gif" => ObjectFormatCode::Gif,
            "bmp" => ObjectFormatCode::Bmp,
            "pict" | "pct" => ObjectFormatCode::Pict,
            "png" => ObjectFormatCode::Png,

            // Executables and scripts
            "exe" | "dll" | "bin" => ObjectFormatCode::Executable,
            "sh" | "bat" | "cmd" | "ps1" => ObjectFormatCode::Script,

            _ => ObjectFormatCode::Undefined,
        }
    }

    /// Check if this format is an audio format.
    pub fn is_audio(&self) -> bool {
        matches!(
            self,
            ObjectFormatCode::Aiff
                | ObjectFormatCode::Wav
                | ObjectFormatCode::Mp3
                | ObjectFormatCode::WmaAudio
                | ObjectFormatCode::OggAudio
                | ObjectFormatCode::AacAudio
                | ObjectFormatCode::FlacAudio
                | ObjectFormatCode::M4aAudio
        )
    }

    /// Check if this format is a video format.
    pub fn is_video(&self) -> bool {
        matches!(
            self,
            ObjectFormatCode::Avi
                | ObjectFormatCode::Mpeg
                | ObjectFormatCode::Asf
                | ObjectFormatCode::WmvVideo
                | ObjectFormatCode::Mp4Container
        )
    }

    /// Check if this format is an image format.
    pub fn is_image(&self) -> bool {
        matches!(
            self,
            ObjectFormatCode::Jpeg
                | ObjectFormatCode::Tiff
                | ObjectFormatCode::Gif
                | ObjectFormatCode::Bmp
                | ObjectFormatCode::Pict
                | ObjectFormatCode::Png
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== OperationCode Tests ====================

    #[test]
    fn operation_code_from_known_codes() {
        assert_eq!(
            OperationCode::from_code(0x1001),
            OperationCode::GetDeviceInfo
        );
        assert_eq!(OperationCode::from_code(0x1002), OperationCode::OpenSession);
        assert_eq!(
            OperationCode::from_code(0x1003),
            OperationCode::CloseSession
        );
        assert_eq!(
            OperationCode::from_code(0x1004),
            OperationCode::GetStorageIds
        );
        assert_eq!(
            OperationCode::from_code(0x1005),
            OperationCode::GetStorageInfo
        );
        assert_eq!(
            OperationCode::from_code(0x1006),
            OperationCode::GetNumObjects
        );
        assert_eq!(
            OperationCode::from_code(0x1007),
            OperationCode::GetObjectHandles
        );
        assert_eq!(
            OperationCode::from_code(0x1008),
            OperationCode::GetObjectInfo
        );
        assert_eq!(OperationCode::from_code(0x1009), OperationCode::GetObject);
        assert_eq!(OperationCode::from_code(0x100A), OperationCode::GetThumb);
        assert_eq!(
            OperationCode::from_code(0x100B),
            OperationCode::DeleteObject
        );
        assert_eq!(
            OperationCode::from_code(0x100C),
            OperationCode::SendObjectInfo
        );
        assert_eq!(OperationCode::from_code(0x100D), OperationCode::SendObject);
        assert_eq!(OperationCode::from_code(0x1019), OperationCode::MoveObject);
        assert_eq!(OperationCode::from_code(0x101A), OperationCode::CopyObject);
        assert_eq!(
            OperationCode::from_code(0x101B),
            OperationCode::GetPartialObject
        );
    }

    #[test]
    fn operation_code_to_known_codes() {
        assert_eq!(OperationCode::GetDeviceInfo.to_code(), 0x1001);
        assert_eq!(OperationCode::OpenSession.to_code(), 0x1002);
        assert_eq!(OperationCode::CloseSession.to_code(), 0x1003);
        assert_eq!(OperationCode::GetStorageIds.to_code(), 0x1004);
        assert_eq!(OperationCode::GetStorageInfo.to_code(), 0x1005);
        assert_eq!(OperationCode::GetNumObjects.to_code(), 0x1006);
        assert_eq!(OperationCode::GetObjectHandles.to_code(), 0x1007);
        assert_eq!(OperationCode::GetObjectInfo.to_code(), 0x1008);
        assert_eq!(OperationCode::GetObject.to_code(), 0x1009);
        assert_eq!(OperationCode::GetThumb.to_code(), 0x100A);
        assert_eq!(OperationCode::DeleteObject.to_code(), 0x100B);
        assert_eq!(OperationCode::SendObjectInfo.to_code(), 0x100C);
        assert_eq!(OperationCode::SendObject.to_code(), 0x100D);
        assert_eq!(OperationCode::MoveObject.to_code(), 0x1019);
        assert_eq!(OperationCode::CopyObject.to_code(), 0x101A);
        assert_eq!(OperationCode::GetPartialObject.to_code(), 0x101B);
    }

    #[test]
    fn operation_code_unknown_roundtrip() {
        let unknown_code = 0x9999;
        let op = OperationCode::from_code(unknown_code);
        assert_eq!(op, OperationCode::Unknown(unknown_code));
        assert_eq!(op.to_code(), unknown_code);
    }

    #[test]
    fn operation_code_known_roundtrip() {
        let codes = [
            OperationCode::GetDeviceInfo,
            OperationCode::OpenSession,
            OperationCode::CloseSession,
            OperationCode::GetStorageIds,
            OperationCode::GetStorageInfo,
            OperationCode::GetNumObjects,
            OperationCode::GetObjectHandles,
            OperationCode::GetObjectInfo,
            OperationCode::GetObject,
            OperationCode::GetThumb,
            OperationCode::DeleteObject,
            OperationCode::SendObjectInfo,
            OperationCode::SendObject,
            OperationCode::MoveObject,
            OperationCode::CopyObject,
            OperationCode::GetPartialObject,
        ];

        for code in codes {
            assert_eq!(OperationCode::from_code(code.to_code()), code);
        }
    }

    // ==================== ResponseCode Tests ====================

    #[test]
    fn response_code_from_known_codes() {
        assert_eq!(ResponseCode::from_code(0x2001), ResponseCode::Ok);
        assert_eq!(ResponseCode::from_code(0x2002), ResponseCode::GeneralError);
        assert_eq!(
            ResponseCode::from_code(0x2003),
            ResponseCode::SessionNotOpen
        );
        assert_eq!(
            ResponseCode::from_code(0x2004),
            ResponseCode::InvalidTransactionId
        );
        assert_eq!(
            ResponseCode::from_code(0x2005),
            ResponseCode::OperationNotSupported
        );
        assert_eq!(
            ResponseCode::from_code(0x2006),
            ResponseCode::ParameterNotSupported
        );
        assert_eq!(
            ResponseCode::from_code(0x2007),
            ResponseCode::IncompleteTransfer
        );
        assert_eq!(
            ResponseCode::from_code(0x2008),
            ResponseCode::InvalidStorageId
        );
        assert_eq!(
            ResponseCode::from_code(0x2009),
            ResponseCode::InvalidObjectHandle
        );
        assert_eq!(ResponseCode::from_code(0x200C), ResponseCode::StoreFull);
        assert_eq!(
            ResponseCode::from_code(0x200D),
            ResponseCode::ObjectWriteProtected
        );
        assert_eq!(ResponseCode::from_code(0x200E), ResponseCode::StoreReadOnly);
        assert_eq!(ResponseCode::from_code(0x200F), ResponseCode::AccessDenied);
        assert_eq!(
            ResponseCode::from_code(0x2010),
            ResponseCode::NoThumbnailPresent
        );
        assert_eq!(ResponseCode::from_code(0x2019), ResponseCode::DeviceBusy);
        assert_eq!(
            ResponseCode::from_code(0x201A),
            ResponseCode::InvalidParentObject
        );
        assert_eq!(
            ResponseCode::from_code(0x201D),
            ResponseCode::InvalidParameter
        );
        assert_eq!(
            ResponseCode::from_code(0x201E),
            ResponseCode::SessionAlreadyOpen
        );
        assert_eq!(
            ResponseCode::from_code(0x201F),
            ResponseCode::TransactionCancelled
        );
        assert_eq!(
            ResponseCode::from_code(0xA809),
            ResponseCode::ObjectTooLarge
        );
    }

    #[test]
    fn response_code_to_known_codes() {
        assert_eq!(ResponseCode::Ok.to_code(), 0x2001);
        assert_eq!(ResponseCode::GeneralError.to_code(), 0x2002);
        assert_eq!(ResponseCode::SessionNotOpen.to_code(), 0x2003);
        assert_eq!(ResponseCode::InvalidTransactionId.to_code(), 0x2004);
        assert_eq!(ResponseCode::OperationNotSupported.to_code(), 0x2005);
        assert_eq!(ResponseCode::ParameterNotSupported.to_code(), 0x2006);
        assert_eq!(ResponseCode::IncompleteTransfer.to_code(), 0x2007);
        assert_eq!(ResponseCode::InvalidStorageId.to_code(), 0x2008);
        assert_eq!(ResponseCode::InvalidObjectHandle.to_code(), 0x2009);
        assert_eq!(ResponseCode::StoreFull.to_code(), 0x200C);
        assert_eq!(ResponseCode::ObjectWriteProtected.to_code(), 0x200D);
        assert_eq!(ResponseCode::StoreReadOnly.to_code(), 0x200E);
        assert_eq!(ResponseCode::AccessDenied.to_code(), 0x200F);
        assert_eq!(ResponseCode::NoThumbnailPresent.to_code(), 0x2010);
        assert_eq!(ResponseCode::DeviceBusy.to_code(), 0x2019);
        assert_eq!(ResponseCode::InvalidParentObject.to_code(), 0x201A);
        assert_eq!(ResponseCode::InvalidParameter.to_code(), 0x201D);
        assert_eq!(ResponseCode::SessionAlreadyOpen.to_code(), 0x201E);
        assert_eq!(ResponseCode::TransactionCancelled.to_code(), 0x201F);
        assert_eq!(ResponseCode::ObjectTooLarge.to_code(), 0xA809);
    }

    #[test]
    fn response_code_unknown_roundtrip() {
        let unknown_code = 0x8888;
        let resp = ResponseCode::from_code(unknown_code);
        assert_eq!(resp, ResponseCode::Unknown(unknown_code));
        assert_eq!(resp.to_code(), unknown_code);
    }

    #[test]
    fn response_code_known_roundtrip() {
        let codes = [
            ResponseCode::Ok,
            ResponseCode::GeneralError,
            ResponseCode::SessionNotOpen,
            ResponseCode::InvalidTransactionId,
            ResponseCode::OperationNotSupported,
            ResponseCode::ParameterNotSupported,
            ResponseCode::IncompleteTransfer,
            ResponseCode::InvalidStorageId,
            ResponseCode::InvalidObjectHandle,
            ResponseCode::StoreFull,
            ResponseCode::ObjectWriteProtected,
            ResponseCode::StoreReadOnly,
            ResponseCode::AccessDenied,
            ResponseCode::NoThumbnailPresent,
            ResponseCode::DeviceBusy,
            ResponseCode::InvalidParentObject,
            ResponseCode::InvalidParameter,
            ResponseCode::SessionAlreadyOpen,
            ResponseCode::TransactionCancelled,
            ResponseCode::ObjectTooLarge,
        ];

        for code in codes {
            assert_eq!(ResponseCode::from_code(code.to_code()), code);
        }
    }

    // ==================== EventCode Tests ====================

    #[test]
    fn event_code_from_known_codes() {
        assert_eq!(EventCode::from_code(0x4002), EventCode::ObjectAdded);
        assert_eq!(EventCode::from_code(0x4003), EventCode::ObjectRemoved);
        assert_eq!(EventCode::from_code(0x4004), EventCode::StoreAdded);
        assert_eq!(EventCode::from_code(0x4005), EventCode::StoreRemoved);
        assert_eq!(EventCode::from_code(0x4006), EventCode::DevicePropChanged);
        assert_eq!(EventCode::from_code(0x4007), EventCode::ObjectInfoChanged);
        assert_eq!(EventCode::from_code(0x4008), EventCode::DeviceInfoChanged);
        assert_eq!(EventCode::from_code(0x400C), EventCode::StorageInfoChanged);
    }

    #[test]
    fn event_code_to_known_codes() {
        assert_eq!(EventCode::ObjectAdded.to_code(), 0x4002);
        assert_eq!(EventCode::ObjectRemoved.to_code(), 0x4003);
        assert_eq!(EventCode::StoreAdded.to_code(), 0x4004);
        assert_eq!(EventCode::StoreRemoved.to_code(), 0x4005);
        assert_eq!(EventCode::DevicePropChanged.to_code(), 0x4006);
        assert_eq!(EventCode::ObjectInfoChanged.to_code(), 0x4007);
        assert_eq!(EventCode::DeviceInfoChanged.to_code(), 0x4008);
        assert_eq!(EventCode::StorageInfoChanged.to_code(), 0x400C);
    }

    #[test]
    fn event_code_unknown_roundtrip() {
        let unknown_code = 0x7777;
        let event = EventCode::from_code(unknown_code);
        assert_eq!(event, EventCode::Unknown(unknown_code));
        assert_eq!(event.to_code(), unknown_code);
    }

    #[test]
    fn event_code_known_roundtrip() {
        let codes = [
            EventCode::ObjectAdded,
            EventCode::ObjectRemoved,
            EventCode::StoreAdded,
            EventCode::StoreRemoved,
            EventCode::DevicePropChanged,
            EventCode::ObjectInfoChanged,
            EventCode::DeviceInfoChanged,
            EventCode::StorageInfoChanged,
        ];

        for code in codes {
            assert_eq!(EventCode::from_code(code.to_code()), code);
        }
    }

    // ==================== ObjectFormatCode Tests ====================

    #[test]
    fn object_format_code_from_known_codes() {
        assert_eq!(
            ObjectFormatCode::from_code(0x3000),
            ObjectFormatCode::Undefined
        );
        assert_eq!(
            ObjectFormatCode::from_code(0x3001),
            ObjectFormatCode::Association
        );
        assert_eq!(
            ObjectFormatCode::from_code(0x3002),
            ObjectFormatCode::Script
        );
        assert_eq!(
            ObjectFormatCode::from_code(0x3003),
            ObjectFormatCode::Executable
        );
        assert_eq!(ObjectFormatCode::from_code(0x3004), ObjectFormatCode::Text);
        assert_eq!(ObjectFormatCode::from_code(0x3005), ObjectFormatCode::Html);
        assert_eq!(ObjectFormatCode::from_code(0x3006), ObjectFormatCode::Dpof);
        assert_eq!(ObjectFormatCode::from_code(0x3007), ObjectFormatCode::Aiff);
        assert_eq!(ObjectFormatCode::from_code(0x3008), ObjectFormatCode::Wav);
        assert_eq!(ObjectFormatCode::from_code(0x3009), ObjectFormatCode::Mp3);
        assert_eq!(ObjectFormatCode::from_code(0x300A), ObjectFormatCode::Avi);
        assert_eq!(ObjectFormatCode::from_code(0x300B), ObjectFormatCode::Mpeg);
        assert_eq!(ObjectFormatCode::from_code(0x300C), ObjectFormatCode::Asf);
        assert_eq!(ObjectFormatCode::from_code(0x3801), ObjectFormatCode::Jpeg);
        assert_eq!(ObjectFormatCode::from_code(0x3804), ObjectFormatCode::Tiff);
        assert_eq!(ObjectFormatCode::from_code(0x3807), ObjectFormatCode::Gif);
        assert_eq!(ObjectFormatCode::from_code(0x3808), ObjectFormatCode::Bmp);
        assert_eq!(ObjectFormatCode::from_code(0x380A), ObjectFormatCode::Pict);
        assert_eq!(ObjectFormatCode::from_code(0x380B), ObjectFormatCode::Png);
        assert_eq!(
            ObjectFormatCode::from_code(0xB901),
            ObjectFormatCode::WmaAudio
        );
        assert_eq!(
            ObjectFormatCode::from_code(0xB902),
            ObjectFormatCode::OggAudio
        );
        assert_eq!(
            ObjectFormatCode::from_code(0xB903),
            ObjectFormatCode::AacAudio
        );
        assert_eq!(
            ObjectFormatCode::from_code(0xB906),
            ObjectFormatCode::FlacAudio
        );
        assert_eq!(
            ObjectFormatCode::from_code(0xB981),
            ObjectFormatCode::WmvVideo
        );
        assert_eq!(
            ObjectFormatCode::from_code(0xB982),
            ObjectFormatCode::Mp4Container
        );
        assert_eq!(
            ObjectFormatCode::from_code(0xB984),
            ObjectFormatCode::M4aAudio
        );
    }

    #[test]
    fn object_format_code_to_known_codes() {
        assert_eq!(ObjectFormatCode::Undefined.to_code(), 0x3000);
        assert_eq!(ObjectFormatCode::Association.to_code(), 0x3001);
        assert_eq!(ObjectFormatCode::Script.to_code(), 0x3002);
        assert_eq!(ObjectFormatCode::Executable.to_code(), 0x3003);
        assert_eq!(ObjectFormatCode::Text.to_code(), 0x3004);
        assert_eq!(ObjectFormatCode::Html.to_code(), 0x3005);
        assert_eq!(ObjectFormatCode::Dpof.to_code(), 0x3006);
        assert_eq!(ObjectFormatCode::Aiff.to_code(), 0x3007);
        assert_eq!(ObjectFormatCode::Wav.to_code(), 0x3008);
        assert_eq!(ObjectFormatCode::Mp3.to_code(), 0x3009);
        assert_eq!(ObjectFormatCode::Avi.to_code(), 0x300A);
        assert_eq!(ObjectFormatCode::Mpeg.to_code(), 0x300B);
        assert_eq!(ObjectFormatCode::Asf.to_code(), 0x300C);
        assert_eq!(ObjectFormatCode::Jpeg.to_code(), 0x3801);
        assert_eq!(ObjectFormatCode::Tiff.to_code(), 0x3804);
        assert_eq!(ObjectFormatCode::Gif.to_code(), 0x3807);
        assert_eq!(ObjectFormatCode::Bmp.to_code(), 0x3808);
        assert_eq!(ObjectFormatCode::Pict.to_code(), 0x380A);
        assert_eq!(ObjectFormatCode::Png.to_code(), 0x380B);
        assert_eq!(ObjectFormatCode::WmaAudio.to_code(), 0xB901);
        assert_eq!(ObjectFormatCode::OggAudio.to_code(), 0xB902);
        assert_eq!(ObjectFormatCode::AacAudio.to_code(), 0xB903);
        assert_eq!(ObjectFormatCode::FlacAudio.to_code(), 0xB906);
        assert_eq!(ObjectFormatCode::WmvVideo.to_code(), 0xB981);
        assert_eq!(ObjectFormatCode::Mp4Container.to_code(), 0xB982);
        assert_eq!(ObjectFormatCode::M4aAudio.to_code(), 0xB984);
    }

    #[test]
    fn object_format_code_unknown_roundtrip() {
        let unknown_code = 0x6666;
        let format = ObjectFormatCode::from_code(unknown_code);
        assert_eq!(format, ObjectFormatCode::Unknown(unknown_code));
        assert_eq!(format.to_code(), unknown_code);
    }

    #[test]
    fn object_format_code_known_roundtrip() {
        let codes = [
            ObjectFormatCode::Undefined,
            ObjectFormatCode::Association,
            ObjectFormatCode::Script,
            ObjectFormatCode::Executable,
            ObjectFormatCode::Text,
            ObjectFormatCode::Html,
            ObjectFormatCode::Dpof,
            ObjectFormatCode::Aiff,
            ObjectFormatCode::Wav,
            ObjectFormatCode::Mp3,
            ObjectFormatCode::Avi,
            ObjectFormatCode::Mpeg,
            ObjectFormatCode::Asf,
            ObjectFormatCode::Jpeg,
            ObjectFormatCode::Tiff,
            ObjectFormatCode::Gif,
            ObjectFormatCode::Bmp,
            ObjectFormatCode::Pict,
            ObjectFormatCode::Png,
            ObjectFormatCode::WmaAudio,
            ObjectFormatCode::OggAudio,
            ObjectFormatCode::AacAudio,
            ObjectFormatCode::FlacAudio,
            ObjectFormatCode::WmvVideo,
            ObjectFormatCode::Mp4Container,
            ObjectFormatCode::M4aAudio,
        ];

        for code in codes {
            assert_eq!(ObjectFormatCode::from_code(code.to_code()), code);
        }
    }

    // ==================== Extension Detection Tests ====================

    #[test]
    fn from_extension_audio_formats() {
        // Lowercase
        assert_eq!(
            ObjectFormatCode::from_extension("mp3"),
            ObjectFormatCode::Mp3
        );
        assert_eq!(
            ObjectFormatCode::from_extension("wav"),
            ObjectFormatCode::Wav
        );
        assert_eq!(
            ObjectFormatCode::from_extension("aiff"),
            ObjectFormatCode::Aiff
        );
        assert_eq!(
            ObjectFormatCode::from_extension("aif"),
            ObjectFormatCode::Aiff
        );
        assert_eq!(
            ObjectFormatCode::from_extension("wma"),
            ObjectFormatCode::WmaAudio
        );
        assert_eq!(
            ObjectFormatCode::from_extension("ogg"),
            ObjectFormatCode::OggAudio
        );
        assert_eq!(
            ObjectFormatCode::from_extension("oga"),
            ObjectFormatCode::OggAudio
        );
        assert_eq!(
            ObjectFormatCode::from_extension("aac"),
            ObjectFormatCode::AacAudio
        );
        assert_eq!(
            ObjectFormatCode::from_extension("flac"),
            ObjectFormatCode::FlacAudio
        );
        assert_eq!(
            ObjectFormatCode::from_extension("m4a"),
            ObjectFormatCode::M4aAudio
        );

        // Uppercase
        assert_eq!(
            ObjectFormatCode::from_extension("MP3"),
            ObjectFormatCode::Mp3
        );
        assert_eq!(
            ObjectFormatCode::from_extension("WAV"),
            ObjectFormatCode::Wav
        );
        assert_eq!(
            ObjectFormatCode::from_extension("FLAC"),
            ObjectFormatCode::FlacAudio
        );

        // Mixed case
        assert_eq!(
            ObjectFormatCode::from_extension("Mp3"),
            ObjectFormatCode::Mp3
        );
        assert_eq!(
            ObjectFormatCode::from_extension("FlaC"),
            ObjectFormatCode::FlacAudio
        );
    }

    #[test]
    fn from_extension_video_formats() {
        assert_eq!(
            ObjectFormatCode::from_extension("avi"),
            ObjectFormatCode::Avi
        );
        assert_eq!(
            ObjectFormatCode::from_extension("mpg"),
            ObjectFormatCode::Mpeg
        );
        assert_eq!(
            ObjectFormatCode::from_extension("mpeg"),
            ObjectFormatCode::Mpeg
        );
        assert_eq!(
            ObjectFormatCode::from_extension("asf"),
            ObjectFormatCode::Asf
        );
        assert_eq!(
            ObjectFormatCode::from_extension("wmv"),
            ObjectFormatCode::WmvVideo
        );
        assert_eq!(
            ObjectFormatCode::from_extension("mp4"),
            ObjectFormatCode::Mp4Container
        );
        assert_eq!(
            ObjectFormatCode::from_extension("m4v"),
            ObjectFormatCode::Mp4Container
        );

        // Uppercase
        assert_eq!(
            ObjectFormatCode::from_extension("AVI"),
            ObjectFormatCode::Avi
        );
        assert_eq!(
            ObjectFormatCode::from_extension("MP4"),
            ObjectFormatCode::Mp4Container
        );
    }

    #[test]
    fn from_extension_image_formats() {
        assert_eq!(
            ObjectFormatCode::from_extension("jpg"),
            ObjectFormatCode::Jpeg
        );
        assert_eq!(
            ObjectFormatCode::from_extension("jpeg"),
            ObjectFormatCode::Jpeg
        );
        assert_eq!(
            ObjectFormatCode::from_extension("tif"),
            ObjectFormatCode::Tiff
        );
        assert_eq!(
            ObjectFormatCode::from_extension("tiff"),
            ObjectFormatCode::Tiff
        );
        assert_eq!(
            ObjectFormatCode::from_extension("gif"),
            ObjectFormatCode::Gif
        );
        assert_eq!(
            ObjectFormatCode::from_extension("bmp"),
            ObjectFormatCode::Bmp
        );
        assert_eq!(
            ObjectFormatCode::from_extension("pict"),
            ObjectFormatCode::Pict
        );
        assert_eq!(
            ObjectFormatCode::from_extension("pct"),
            ObjectFormatCode::Pict
        );
        assert_eq!(
            ObjectFormatCode::from_extension("png"),
            ObjectFormatCode::Png
        );

        // Uppercase
        assert_eq!(
            ObjectFormatCode::from_extension("JPG"),
            ObjectFormatCode::Jpeg
        );
        assert_eq!(
            ObjectFormatCode::from_extension("JPEG"),
            ObjectFormatCode::Jpeg
        );
        assert_eq!(
            ObjectFormatCode::from_extension("PNG"),
            ObjectFormatCode::Png
        );
    }

    #[test]
    fn from_extension_text_formats() {
        assert_eq!(
            ObjectFormatCode::from_extension("txt"),
            ObjectFormatCode::Text
        );
        assert_eq!(
            ObjectFormatCode::from_extension("html"),
            ObjectFormatCode::Html
        );
        assert_eq!(
            ObjectFormatCode::from_extension("htm"),
            ObjectFormatCode::Html
        );
    }

    #[test]
    fn from_extension_executable_formats() {
        assert_eq!(
            ObjectFormatCode::from_extension("exe"),
            ObjectFormatCode::Executable
        );
        assert_eq!(
            ObjectFormatCode::from_extension("dll"),
            ObjectFormatCode::Executable
        );
        assert_eq!(
            ObjectFormatCode::from_extension("bin"),
            ObjectFormatCode::Executable
        );
        assert_eq!(
            ObjectFormatCode::from_extension("sh"),
            ObjectFormatCode::Script
        );
        assert_eq!(
            ObjectFormatCode::from_extension("bat"),
            ObjectFormatCode::Script
        );
        assert_eq!(
            ObjectFormatCode::from_extension("cmd"),
            ObjectFormatCode::Script
        );
        assert_eq!(
            ObjectFormatCode::from_extension("ps1"),
            ObjectFormatCode::Script
        );
    }

    #[test]
    fn from_extension_unknown() {
        assert_eq!(
            ObjectFormatCode::from_extension("xyz"),
            ObjectFormatCode::Undefined
        );
        assert_eq!(
            ObjectFormatCode::from_extension("unknown"),
            ObjectFormatCode::Undefined
        );
        assert_eq!(
            ObjectFormatCode::from_extension(""),
            ObjectFormatCode::Undefined
        );
        assert_eq!(
            ObjectFormatCode::from_extension("rs"),
            ObjectFormatCode::Undefined
        );
    }

    // ==================== is_audio/is_video/is_image Tests ====================

    #[test]
    fn is_audio_returns_true_for_audio_formats() {
        assert!(ObjectFormatCode::Mp3.is_audio());
        assert!(ObjectFormatCode::Wav.is_audio());
        assert!(ObjectFormatCode::Aiff.is_audio());
        assert!(ObjectFormatCode::WmaAudio.is_audio());
        assert!(ObjectFormatCode::OggAudio.is_audio());
        assert!(ObjectFormatCode::AacAudio.is_audio());
        assert!(ObjectFormatCode::FlacAudio.is_audio());
        assert!(ObjectFormatCode::M4aAudio.is_audio());
    }

    #[test]
    fn is_audio_returns_false_for_non_audio_formats() {
        assert!(!ObjectFormatCode::Jpeg.is_audio());
        assert!(!ObjectFormatCode::Mp4Container.is_audio());
        assert!(!ObjectFormatCode::Text.is_audio());
        assert!(!ObjectFormatCode::Association.is_audio());
        assert!(!ObjectFormatCode::Unknown(0x1234).is_audio());
    }

    #[test]
    fn is_video_returns_true_for_video_formats() {
        assert!(ObjectFormatCode::Avi.is_video());
        assert!(ObjectFormatCode::Mpeg.is_video());
        assert!(ObjectFormatCode::Asf.is_video());
        assert!(ObjectFormatCode::WmvVideo.is_video());
        assert!(ObjectFormatCode::Mp4Container.is_video());
    }

    #[test]
    fn is_video_returns_false_for_non_video_formats() {
        assert!(!ObjectFormatCode::Mp3.is_video());
        assert!(!ObjectFormatCode::Jpeg.is_video());
        assert!(!ObjectFormatCode::Text.is_video());
        assert!(!ObjectFormatCode::Association.is_video());
        assert!(!ObjectFormatCode::Unknown(0x1234).is_video());
    }

    #[test]
    fn is_image_returns_true_for_image_formats() {
        assert!(ObjectFormatCode::Jpeg.is_image());
        assert!(ObjectFormatCode::Tiff.is_image());
        assert!(ObjectFormatCode::Gif.is_image());
        assert!(ObjectFormatCode::Bmp.is_image());
        assert!(ObjectFormatCode::Pict.is_image());
        assert!(ObjectFormatCode::Png.is_image());
    }

    #[test]
    fn is_image_returns_false_for_non_image_formats() {
        assert!(!ObjectFormatCode::Mp3.is_image());
        assert!(!ObjectFormatCode::Mp4Container.is_image());
        assert!(!ObjectFormatCode::Text.is_image());
        assert!(!ObjectFormatCode::Association.is_image());
        assert!(!ObjectFormatCode::Unknown(0x1234).is_image());
    }

    #[test]
    fn format_categories_are_mutually_exclusive() {
        // Test that audio, video, and image formats don't overlap
        let all_formats = [
            ObjectFormatCode::Undefined,
            ObjectFormatCode::Association,
            ObjectFormatCode::Script,
            ObjectFormatCode::Executable,
            ObjectFormatCode::Text,
            ObjectFormatCode::Html,
            ObjectFormatCode::Dpof,
            ObjectFormatCode::Aiff,
            ObjectFormatCode::Wav,
            ObjectFormatCode::Mp3,
            ObjectFormatCode::Avi,
            ObjectFormatCode::Mpeg,
            ObjectFormatCode::Asf,
            ObjectFormatCode::Jpeg,
            ObjectFormatCode::Tiff,
            ObjectFormatCode::Gif,
            ObjectFormatCode::Bmp,
            ObjectFormatCode::Pict,
            ObjectFormatCode::Png,
            ObjectFormatCode::WmaAudio,
            ObjectFormatCode::OggAudio,
            ObjectFormatCode::AacAudio,
            ObjectFormatCode::FlacAudio,
            ObjectFormatCode::WmvVideo,
            ObjectFormatCode::Mp4Container,
            ObjectFormatCode::M4aAudio,
        ];

        for format in all_formats {
            let categories = [format.is_audio(), format.is_video(), format.is_image()];
            let true_count = categories.iter().filter(|&&b| b).count();
            assert!(
                true_count <= 1,
                "{:?} belongs to multiple categories: audio={}, video={}, image={}",
                format,
                format.is_audio(),
                format.is_video(),
                format.is_image()
            );
        }
    }
}
