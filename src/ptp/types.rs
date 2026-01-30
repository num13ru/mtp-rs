//! MTP/PTP data structures for device, storage, and object information.
//!
//! This module provides high-level structures for parsing protocol responses:
//! - [`DeviceInfo`]: Device capabilities and identification
//! - [`StorageInfo`]: Storage characteristics and capacity
//! - [`ObjectInfo`]: File/folder metadata

use super::pack::{
    pack_datetime, pack_string, pack_u16, pack_u32, unpack_datetime, unpack_string, unpack_u16,
    unpack_u16_array, unpack_u32, unpack_u64, DateTime,
};
use super::{EventCode, ObjectFormatCode, ObjectHandle, OperationCode, StorageId};

// =============================================================================
// Storage Type Enum
// =============================================================================

/// Type of storage medium.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageType {
    /// Undefined storage type.
    #[default]
    Undefined,
    /// Fixed ROM (e.g., internal flash).
    FixedRom,
    /// Removable ROM.
    RemovableRom,
    /// Fixed RAM.
    FixedRam,
    /// Removable RAM (e.g., SD card).
    RemovableRam,
    /// Unknown storage type code.
    Unknown(u16),
}

impl StorageType {
    /// Convert a raw u16 code to a StorageType.
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => StorageType::Undefined,
            1 => StorageType::FixedRom,
            2 => StorageType::RemovableRom,
            3 => StorageType::FixedRam,
            4 => StorageType::RemovableRam,
            _ => StorageType::Unknown(code),
        }
    }

    /// Convert a StorageType to its raw u16 value.
    pub fn to_code(self) -> u16 {
        match self {
            StorageType::Undefined => 0,
            StorageType::FixedRom => 1,
            StorageType::RemovableRom => 2,
            StorageType::FixedRam => 3,
            StorageType::RemovableRam => 4,
            StorageType::Unknown(code) => code,
        }
    }
}

// =============================================================================
// Filesystem Type Enum
// =============================================================================

/// Type of filesystem on the storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilesystemType {
    /// Undefined filesystem type.
    #[default]
    Undefined,
    /// Generic flat filesystem (no folders).
    GenericFlat,
    /// Generic hierarchical filesystem (with folders).
    GenericHierarchical,
    /// DCF (Design rule for Camera File system).
    Dcf,
    /// Unknown filesystem type code.
    Unknown(u16),
}

impl FilesystemType {
    /// Convert a raw u16 code to a FilesystemType.
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => FilesystemType::Undefined,
            1 => FilesystemType::GenericFlat,
            2 => FilesystemType::GenericHierarchical,
            3 => FilesystemType::Dcf,
            _ => FilesystemType::Unknown(code),
        }
    }

    /// Convert a FilesystemType to its raw u16 value.
    pub fn to_code(self) -> u16 {
        match self {
            FilesystemType::Undefined => 0,
            FilesystemType::GenericFlat => 1,
            FilesystemType::GenericHierarchical => 2,
            FilesystemType::Dcf => 3,
            FilesystemType::Unknown(code) => code,
        }
    }
}

// =============================================================================
// Access Capability Enum
// =============================================================================

/// Access capability of the storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccessCapability {
    /// Read-write access.
    #[default]
    ReadWrite,
    /// Read-only, deletion not allowed.
    ReadOnlyWithoutDeletion,
    /// Read-only, deletion allowed.
    ReadOnlyWithDeletion,
    /// Unknown access capability code.
    Unknown(u16),
}

impl AccessCapability {
    /// Convert a raw u16 code to an AccessCapability.
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => AccessCapability::ReadWrite,
            1 => AccessCapability::ReadOnlyWithoutDeletion,
            2 => AccessCapability::ReadOnlyWithDeletion,
            _ => AccessCapability::Unknown(code),
        }
    }

    /// Convert an AccessCapability to its raw u16 value.
    pub fn to_code(self) -> u16 {
        match self {
            AccessCapability::ReadWrite => 0,
            AccessCapability::ReadOnlyWithoutDeletion => 1,
            AccessCapability::ReadOnlyWithDeletion => 2,
            AccessCapability::Unknown(code) => code,
        }
    }
}

// =============================================================================
// Protection Status Enum
// =============================================================================

/// Protection status of an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProtectionStatus {
    /// No protection.
    #[default]
    None,
    /// Read-only protection.
    ReadOnly,
    /// Unknown protection status code.
    Unknown(u16),
}

impl ProtectionStatus {
    /// Convert a raw u16 code to a ProtectionStatus.
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => ProtectionStatus::None,
            1 => ProtectionStatus::ReadOnly,
            _ => ProtectionStatus::Unknown(code),
        }
    }

    /// Convert a ProtectionStatus to its raw u16 value.
    pub fn to_code(self) -> u16 {
        match self {
            ProtectionStatus::None => 0,
            ProtectionStatus::ReadOnly => 1,
            ProtectionStatus::Unknown(code) => code,
        }
    }
}

// =============================================================================
// Association Type Enum
// =============================================================================

/// Association type for objects (folder/container type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssociationType {
    /// No association (regular file).
    #[default]
    None,
    /// Generic folder.
    GenericFolder,
    /// Unknown association type code.
    Unknown(u16),
}

impl AssociationType {
    /// Convert a raw u16 code to an AssociationType.
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => AssociationType::None,
            1 => AssociationType::GenericFolder,
            _ => AssociationType::Unknown(code),
        }
    }

    /// Convert an AssociationType to its raw u16 value.
    pub fn to_code(self) -> u16 {
        match self {
            AssociationType::None => 0,
            AssociationType::GenericFolder => 1,
            AssociationType::Unknown(code) => code,
        }
    }
}

// =============================================================================
// DeviceInfo Structure
// =============================================================================

/// Device information returned by GetDeviceInfo.
///
/// Contains device capabilities, manufacturer info, and supported operations.
#[derive(Debug, Clone, Default)]
pub struct DeviceInfo {
    /// PTP standard version (e.g., 100 = v1.00).
    pub standard_version: u16,
    /// Vendor extension ID (0 = no extension).
    pub vendor_extension_id: u32,
    /// Vendor extension version.
    pub vendor_extension_version: u16,
    /// Vendor extension description.
    pub vendor_extension_desc: String,
    /// Functional mode (0 = standard).
    pub functional_mode: u16,
    /// Operations supported by the device.
    pub operations_supported: Vec<OperationCode>,
    /// Events supported by the device.
    pub events_supported: Vec<EventCode>,
    /// Device properties supported.
    pub device_properties_supported: Vec<u16>,
    /// Object formats the device can capture/create.
    pub capture_formats: Vec<ObjectFormatCode>,
    /// Object formats the device can play/display.
    pub playback_formats: Vec<ObjectFormatCode>,
    /// Manufacturer name.
    pub manufacturer: String,
    /// Device model name.
    pub model: String,
    /// Device version string.
    pub device_version: String,
    /// Device serial number.
    pub serial_number: String,
}

impl DeviceInfo {
    /// Parse DeviceInfo from a byte buffer.
    ///
    /// The buffer should contain the DeviceInfo dataset as returned by GetDeviceInfo.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::Error> {
        let mut offset = 0;

        // 1. StandardVersion (u16)
        let standard_version = unpack_u16(&buf[offset..])?;
        offset += 2;

        // 2. VendorExtensionID (u32)
        let vendor_extension_id = unpack_u32(&buf[offset..])?;
        offset += 4;

        // 3. VendorExtensionVersion (u16)
        let vendor_extension_version = unpack_u16(&buf[offset..])?;
        offset += 2;

        // 4. VendorExtensionDesc (string)
        let (vendor_extension_desc, consumed) = unpack_string(&buf[offset..])?;
        offset += consumed;

        // 5. FunctionalMode (u16)
        let functional_mode = unpack_u16(&buf[offset..])?;
        offset += 2;

        // 6. OperationsSupported (u16 array)
        let (ops_raw, consumed) = unpack_u16_array(&buf[offset..])?;
        let operations_supported: Vec<OperationCode> =
            ops_raw.into_iter().map(OperationCode::from_code).collect();
        offset += consumed;

        // 7. EventsSupported (u16 array)
        let (events_raw, consumed) = unpack_u16_array(&buf[offset..])?;
        let events_supported: Vec<EventCode> =
            events_raw.into_iter().map(EventCode::from_code).collect();
        offset += consumed;

        // 8. DevicePropertiesSupported (u16 array)
        let (device_properties_supported, consumed) = unpack_u16_array(&buf[offset..])?;
        offset += consumed;

        // 9. CaptureFormats (u16 array)
        let (capture_raw, consumed) = unpack_u16_array(&buf[offset..])?;
        let capture_formats: Vec<ObjectFormatCode> = capture_raw
            .into_iter()
            .map(ObjectFormatCode::from_code)
            .collect();
        offset += consumed;

        // 10. PlaybackFormats (u16 array)
        let (playback_raw, consumed) = unpack_u16_array(&buf[offset..])?;
        let playback_formats: Vec<ObjectFormatCode> = playback_raw
            .into_iter()
            .map(ObjectFormatCode::from_code)
            .collect();
        offset += consumed;

        // 11. Manufacturer (string)
        let (manufacturer, consumed) = unpack_string(&buf[offset..])?;
        offset += consumed;

        // 12. Model (string)
        let (model, consumed) = unpack_string(&buf[offset..])?;
        offset += consumed;

        // 13. DeviceVersion (string)
        let (device_version, consumed) = unpack_string(&buf[offset..])?;
        offset += consumed;

        // 14. SerialNumber (string)
        let (serial_number, _consumed) = unpack_string(&buf[offset..])?;

        Ok(DeviceInfo {
            standard_version,
            vendor_extension_id,
            vendor_extension_version,
            vendor_extension_desc,
            functional_mode,
            operations_supported,
            events_supported,
            device_properties_supported,
            capture_formats,
            playback_formats,
            manufacturer,
            model,
            device_version,
            serial_number,
        })
    }
}

// =============================================================================
// StorageInfo Structure
// =============================================================================

/// Storage information returned by GetStorageInfo.
///
/// Contains storage capacity, type, and access information.
#[derive(Debug, Clone, Default)]
pub struct StorageInfo {
    /// Type of storage medium.
    pub storage_type: StorageType,
    /// Type of filesystem.
    pub filesystem_type: FilesystemType,
    /// Access capability.
    pub access_capability: AccessCapability,
    /// Maximum storage capacity in bytes.
    pub max_capacity: u64,
    /// Free space in bytes.
    pub free_space_bytes: u64,
    /// Free space in number of objects (0xFFFFFFFF if unknown).
    pub free_space_objects: u32,
    /// Storage description string.
    pub description: String,
    /// Volume identifier/label.
    pub volume_identifier: String,
}

impl StorageInfo {
    /// Parse StorageInfo from a byte buffer.
    ///
    /// The buffer should contain the StorageInfo dataset as returned by GetStorageInfo.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::Error> {
        let mut offset = 0;

        // 1. StorageType (u16)
        let storage_type = StorageType::from_code(unpack_u16(&buf[offset..])?);
        offset += 2;

        // 2. FilesystemType (u16)
        let filesystem_type = FilesystemType::from_code(unpack_u16(&buf[offset..])?);
        offset += 2;

        // 3. AccessCapability (u16)
        let access_capability = AccessCapability::from_code(unpack_u16(&buf[offset..])?);
        offset += 2;

        // 4. MaxCapacity (u64)
        let max_capacity = unpack_u64(&buf[offset..])?;
        offset += 8;

        // 5. FreeSpaceInBytes (u64)
        let free_space_bytes = unpack_u64(&buf[offset..])?;
        offset += 8;

        // 6. FreeSpaceInObjects (u32)
        let free_space_objects = unpack_u32(&buf[offset..])?;
        offset += 4;

        // 7. StorageDescription (string)
        let (description, consumed) = unpack_string(&buf[offset..])?;
        offset += consumed;

        // 8. VolumeIdentifier (string)
        let (volume_identifier, _consumed) = unpack_string(&buf[offset..])?;

        Ok(StorageInfo {
            storage_type,
            filesystem_type,
            access_capability,
            max_capacity,
            free_space_bytes,
            free_space_objects,
            description,
            volume_identifier,
        })
    }
}

// =============================================================================
// ObjectInfo Structure
// =============================================================================

/// Object information returned by GetObjectInfo.
///
/// Contains file/folder metadata including name, size, timestamps, and hierarchy info.
#[derive(Debug, Clone, Default)]
pub struct ObjectInfo {
    /// Storage containing this object.
    pub storage_id: StorageId,
    /// Object format code.
    pub format: ObjectFormatCode,
    /// Protection status.
    pub protection_status: ProtectionStatus,
    /// Object size in bytes.
    ///
    /// Note: Protocol uses u32, but we store as u64. Values of 0xFFFFFFFF indicate
    /// the object is larger than 4GB (use GetObjectPropValue for actual size).
    pub size: u64,
    /// Thumbnail format.
    pub thumb_format: ObjectFormatCode,
    /// Thumbnail size in bytes.
    pub thumb_size: u32,
    /// Thumbnail width in pixels.
    pub thumb_width: u32,
    /// Thumbnail height in pixels.
    pub thumb_height: u32,
    /// Image width in pixels.
    pub image_width: u32,
    /// Image height in pixels.
    pub image_height: u32,
    /// Image bit depth.
    pub image_bit_depth: u32,
    /// Parent object handle (ROOT for root-level objects).
    pub parent: ObjectHandle,
    /// Association type (folder type).
    pub association_type: AssociationType,
    /// Association description.
    pub association_desc: u32,
    /// Sequence number.
    pub sequence_number: u32,
    /// Filename.
    pub filename: String,
    /// Creation timestamp.
    pub created: Option<DateTime>,
    /// Modification timestamp.
    pub modified: Option<DateTime>,
    /// Keywords string.
    pub keywords: String,
}

impl ObjectInfo {
    /// Parse ObjectInfo from a byte buffer.
    ///
    /// The buffer should contain the ObjectInfo dataset as returned by GetObjectInfo.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::Error> {
        let mut offset = 0;

        // 1. StorageID (u32)
        let storage_id = StorageId(unpack_u32(&buf[offset..])?);
        offset += 4;

        // 2. ObjectFormat (u16)
        let format = ObjectFormatCode::from_code(unpack_u16(&buf[offset..])?);
        offset += 2;

        // 3. ProtectionStatus (u16)
        let protection_status = ProtectionStatus::from_code(unpack_u16(&buf[offset..])?);
        offset += 2;

        // 4. ObjectCompressedSize (u32) - stored as u64
        let size = unpack_u32(&buf[offset..])? as u64;
        offset += 4;

        // 5. ThumbFormat (u16)
        let thumb_format = ObjectFormatCode::from_code(unpack_u16(&buf[offset..])?);
        offset += 2;

        // 6. ThumbCompressedSize (u32)
        let thumb_size = unpack_u32(&buf[offset..])?;
        offset += 4;

        // 7. ThumbPixWidth (u32)
        let thumb_width = unpack_u32(&buf[offset..])?;
        offset += 4;

        // 8. ThumbPixHeight (u32)
        let thumb_height = unpack_u32(&buf[offset..])?;
        offset += 4;

        // 9. ImagePixWidth (u32)
        let image_width = unpack_u32(&buf[offset..])?;
        offset += 4;

        // 10. ImagePixHeight (u32)
        let image_height = unpack_u32(&buf[offset..])?;
        offset += 4;

        // 11. ImageBitDepth (u32)
        let image_bit_depth = unpack_u32(&buf[offset..])?;
        offset += 4;

        // 12. ParentObject (u32)
        let parent = ObjectHandle(unpack_u32(&buf[offset..])?);
        offset += 4;

        // 13. AssociationType (u16)
        let association_type = AssociationType::from_code(unpack_u16(&buf[offset..])?);
        offset += 2;

        // 14. AssociationDesc (u32)
        let association_desc = unpack_u32(&buf[offset..])?;
        offset += 4;

        // 15. SequenceNumber (u32)
        let sequence_number = unpack_u32(&buf[offset..])?;
        offset += 4;

        // 16. Filename (string)
        let (filename, consumed) = unpack_string(&buf[offset..])?;
        offset += consumed;

        // 17. DateCreated (datetime string)
        let (created, consumed) = unpack_datetime(&buf[offset..])?;
        offset += consumed;

        // 18. DateModified (datetime string)
        let (modified, consumed) = unpack_datetime(&buf[offset..])?;
        offset += consumed;

        // 19. Keywords (string)
        let (keywords, _consumed) = unpack_string(&buf[offset..])?;

        Ok(ObjectInfo {
            storage_id,
            format,
            protection_status,
            size,
            thumb_format,
            thumb_size,
            thumb_width,
            thumb_height,
            image_width,
            image_height,
            image_bit_depth,
            parent,
            association_type,
            association_desc,
            sequence_number,
            filename,
            created,
            modified,
            keywords,
        })
    }

    /// Serialize ObjectInfo to a byte buffer.
    ///
    /// Used for SendObjectInfo operation.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // 1. StorageID (u32)
        buf.extend_from_slice(&pack_u32(self.storage_id.0));

        // 2. ObjectFormat (u16)
        buf.extend_from_slice(&pack_u16(self.format.to_code()));

        // 3. ProtectionStatus (u16)
        buf.extend_from_slice(&pack_u16(self.protection_status.to_code()));

        // 4. ObjectCompressedSize (u32) - cap at u32::MAX for >4GB files
        let size_u32 = if self.size > u32::MAX as u64 {
            u32::MAX
        } else {
            self.size as u32
        };
        buf.extend_from_slice(&pack_u32(size_u32));

        // 5. ThumbFormat (u16)
        buf.extend_from_slice(&pack_u16(self.thumb_format.to_code()));

        // 6. ThumbCompressedSize (u32)
        buf.extend_from_slice(&pack_u32(self.thumb_size));

        // 7. ThumbPixWidth (u32)
        buf.extend_from_slice(&pack_u32(self.thumb_width));

        // 8. ThumbPixHeight (u32)
        buf.extend_from_slice(&pack_u32(self.thumb_height));

        // 9. ImagePixWidth (u32)
        buf.extend_from_slice(&pack_u32(self.image_width));

        // 10. ImagePixHeight (u32)
        buf.extend_from_slice(&pack_u32(self.image_height));

        // 11. ImageBitDepth (u32)
        buf.extend_from_slice(&pack_u32(self.image_bit_depth));

        // 12. ParentObject (u32)
        buf.extend_from_slice(&pack_u32(self.parent.0));

        // 13. AssociationType (u16)
        buf.extend_from_slice(&pack_u16(self.association_type.to_code()));

        // 14. AssociationDesc (u32)
        buf.extend_from_slice(&pack_u32(self.association_desc));

        // 15. SequenceNumber (u32)
        buf.extend_from_slice(&pack_u32(self.sequence_number));

        // 16. Filename (string)
        buf.extend_from_slice(&pack_string(&self.filename));

        // 17. DateCreated (datetime string)
        if let Some(dt) = &self.created {
            buf.extend_from_slice(&pack_datetime(dt));
        } else {
            buf.push(0x00); // Empty string
        }

        // 18. DateModified (datetime string)
        if let Some(dt) = &self.modified {
            buf.extend_from_slice(&pack_datetime(dt));
        } else {
            buf.push(0x00); // Empty string
        }

        // 19. Keywords (string)
        buf.extend_from_slice(&pack_string(&self.keywords));

        buf
    }

    /// Check if this object is a folder.
    ///
    /// Returns true if the format is Association or the association type is GenericFolder.
    pub fn is_folder(&self) -> bool {
        self.format == ObjectFormatCode::Association
            || self.association_type == AssociationType::GenericFolder
    }

    /// Check if this object is a file.
    ///
    /// Returns true if this is not a folder.
    pub fn is_file(&self) -> bool {
        !self.is_folder()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ptp::pack::pack_u16_array;

    // =========================================================================
    // StorageType Tests
    // =========================================================================

    #[test]
    fn storage_type_from_code() {
        assert_eq!(StorageType::from_code(0), StorageType::Undefined);
        assert_eq!(StorageType::from_code(1), StorageType::FixedRom);
        assert_eq!(StorageType::from_code(2), StorageType::RemovableRom);
        assert_eq!(StorageType::from_code(3), StorageType::FixedRam);
        assert_eq!(StorageType::from_code(4), StorageType::RemovableRam);
        assert_eq!(StorageType::from_code(99), StorageType::Unknown(99));
    }

    #[test]
    fn storage_type_to_code() {
        assert_eq!(StorageType::Undefined.to_code(), 0);
        assert_eq!(StorageType::FixedRom.to_code(), 1);
        assert_eq!(StorageType::RemovableRom.to_code(), 2);
        assert_eq!(StorageType::FixedRam.to_code(), 3);
        assert_eq!(StorageType::RemovableRam.to_code(), 4);
        assert_eq!(StorageType::Unknown(99).to_code(), 99);
    }

    #[test]
    fn storage_type_roundtrip() {
        let types = [
            StorageType::Undefined,
            StorageType::FixedRom,
            StorageType::RemovableRom,
            StorageType::FixedRam,
            StorageType::RemovableRam,
        ];
        for t in types {
            assert_eq!(StorageType::from_code(t.to_code()), t);
        }
    }

    #[test]
    fn storage_type_default() {
        assert_eq!(StorageType::default(), StorageType::Undefined);
    }

    // =========================================================================
    // FilesystemType Tests
    // =========================================================================

    #[test]
    fn filesystem_type_from_code() {
        assert_eq!(FilesystemType::from_code(0), FilesystemType::Undefined);
        assert_eq!(FilesystemType::from_code(1), FilesystemType::GenericFlat);
        assert_eq!(
            FilesystemType::from_code(2),
            FilesystemType::GenericHierarchical
        );
        assert_eq!(FilesystemType::from_code(3), FilesystemType::Dcf);
        assert_eq!(FilesystemType::from_code(99), FilesystemType::Unknown(99));
    }

    #[test]
    fn filesystem_type_to_code() {
        assert_eq!(FilesystemType::Undefined.to_code(), 0);
        assert_eq!(FilesystemType::GenericFlat.to_code(), 1);
        assert_eq!(FilesystemType::GenericHierarchical.to_code(), 2);
        assert_eq!(FilesystemType::Dcf.to_code(), 3);
        assert_eq!(FilesystemType::Unknown(99).to_code(), 99);
    }

    #[test]
    fn filesystem_type_roundtrip() {
        let types = [
            FilesystemType::Undefined,
            FilesystemType::GenericFlat,
            FilesystemType::GenericHierarchical,
            FilesystemType::Dcf,
        ];
        for t in types {
            assert_eq!(FilesystemType::from_code(t.to_code()), t);
        }
    }

    #[test]
    fn filesystem_type_default() {
        assert_eq!(FilesystemType::default(), FilesystemType::Undefined);
    }

    // =========================================================================
    // AccessCapability Tests
    // =========================================================================

    #[test]
    fn access_capability_from_code() {
        assert_eq!(AccessCapability::from_code(0), AccessCapability::ReadWrite);
        assert_eq!(
            AccessCapability::from_code(1),
            AccessCapability::ReadOnlyWithoutDeletion
        );
        assert_eq!(
            AccessCapability::from_code(2),
            AccessCapability::ReadOnlyWithDeletion
        );
        assert_eq!(
            AccessCapability::from_code(99),
            AccessCapability::Unknown(99)
        );
    }

    #[test]
    fn access_capability_to_code() {
        assert_eq!(AccessCapability::ReadWrite.to_code(), 0);
        assert_eq!(AccessCapability::ReadOnlyWithoutDeletion.to_code(), 1);
        assert_eq!(AccessCapability::ReadOnlyWithDeletion.to_code(), 2);
        assert_eq!(AccessCapability::Unknown(99).to_code(), 99);
    }

    #[test]
    fn access_capability_roundtrip() {
        let caps = [
            AccessCapability::ReadWrite,
            AccessCapability::ReadOnlyWithoutDeletion,
            AccessCapability::ReadOnlyWithDeletion,
        ];
        for c in caps {
            assert_eq!(AccessCapability::from_code(c.to_code()), c);
        }
    }

    #[test]
    fn access_capability_default() {
        assert_eq!(AccessCapability::default(), AccessCapability::ReadWrite);
    }

    // =========================================================================
    // ProtectionStatus Tests
    // =========================================================================

    #[test]
    fn protection_status_from_code() {
        assert_eq!(ProtectionStatus::from_code(0), ProtectionStatus::None);
        assert_eq!(ProtectionStatus::from_code(1), ProtectionStatus::ReadOnly);
        assert_eq!(
            ProtectionStatus::from_code(99),
            ProtectionStatus::Unknown(99)
        );
    }

    #[test]
    fn protection_status_to_code() {
        assert_eq!(ProtectionStatus::None.to_code(), 0);
        assert_eq!(ProtectionStatus::ReadOnly.to_code(), 1);
        assert_eq!(ProtectionStatus::Unknown(99).to_code(), 99);
    }

    #[test]
    fn protection_status_roundtrip() {
        let statuses = [ProtectionStatus::None, ProtectionStatus::ReadOnly];
        for s in statuses {
            assert_eq!(ProtectionStatus::from_code(s.to_code()), s);
        }
    }

    #[test]
    fn protection_status_default() {
        assert_eq!(ProtectionStatus::default(), ProtectionStatus::None);
    }

    // =========================================================================
    // AssociationType Tests
    // =========================================================================

    #[test]
    fn association_type_from_code() {
        assert_eq!(AssociationType::from_code(0), AssociationType::None);
        assert_eq!(
            AssociationType::from_code(1),
            AssociationType::GenericFolder
        );
        assert_eq!(AssociationType::from_code(99), AssociationType::Unknown(99));
    }

    #[test]
    fn association_type_to_code() {
        assert_eq!(AssociationType::None.to_code(), 0);
        assert_eq!(AssociationType::GenericFolder.to_code(), 1);
        assert_eq!(AssociationType::Unknown(99).to_code(), 99);
    }

    #[test]
    fn association_type_roundtrip() {
        let types = [AssociationType::None, AssociationType::GenericFolder];
        for t in types {
            assert_eq!(AssociationType::from_code(t.to_code()), t);
        }
    }

    #[test]
    fn association_type_default() {
        assert_eq!(AssociationType::default(), AssociationType::None);
    }

    // =========================================================================
    // DeviceInfo Tests
    // =========================================================================

    fn build_minimal_device_info_bytes() -> Vec<u8> {
        let mut buf = Vec::new();

        // StandardVersion: 100 (v1.00)
        buf.extend_from_slice(&pack_u16(100));
        // VendorExtensionID: 0
        buf.extend_from_slice(&pack_u32(0));
        // VendorExtensionVersion: 0
        buf.extend_from_slice(&pack_u16(0));
        // VendorExtensionDesc: empty string
        buf.push(0x00);
        // FunctionalMode: 0
        buf.extend_from_slice(&pack_u16(0));
        // OperationsSupported: empty array
        buf.extend_from_slice(&pack_u16_array(&[]));
        // EventsSupported: empty array
        buf.extend_from_slice(&pack_u16_array(&[]));
        // DevicePropertiesSupported: empty array
        buf.extend_from_slice(&pack_u16_array(&[]));
        // CaptureFormats: empty array
        buf.extend_from_slice(&pack_u16_array(&[]));
        // PlaybackFormats: empty array
        buf.extend_from_slice(&pack_u16_array(&[]));
        // Manufacturer: empty string
        buf.push(0x00);
        // Model: empty string
        buf.push(0x00);
        // DeviceVersion: empty string
        buf.push(0x00);
        // SerialNumber: empty string
        buf.push(0x00);

        buf
    }

    #[test]
    fn device_info_parse_minimal() {
        let buf = build_minimal_device_info_bytes();
        let info = DeviceInfo::from_bytes(&buf).unwrap();

        assert_eq!(info.standard_version, 100);
        assert_eq!(info.vendor_extension_id, 0);
        assert_eq!(info.vendor_extension_version, 0);
        assert_eq!(info.vendor_extension_desc, "");
        assert_eq!(info.functional_mode, 0);
        assert!(info.operations_supported.is_empty());
        assert!(info.events_supported.is_empty());
        assert!(info.device_properties_supported.is_empty());
        assert!(info.capture_formats.is_empty());
        assert!(info.playback_formats.is_empty());
        assert_eq!(info.manufacturer, "");
        assert_eq!(info.model, "");
        assert_eq!(info.device_version, "");
        assert_eq!(info.serial_number, "");
    }

    fn build_full_device_info_bytes() -> Vec<u8> {
        let mut buf = Vec::new();

        // StandardVersion: 100 (v1.00)
        buf.extend_from_slice(&pack_u16(100));
        // VendorExtensionID: 0x00000006 (Microsoft)
        buf.extend_from_slice(&pack_u32(6));
        // VendorExtensionVersion: 100
        buf.extend_from_slice(&pack_u16(100));
        // VendorExtensionDesc: "microsoft.com: 1.0"
        buf.extend_from_slice(&pack_string("microsoft.com: 1.0"));
        // FunctionalMode: 0
        buf.extend_from_slice(&pack_u16(0));
        // OperationsSupported: [GetDeviceInfo, OpenSession, CloseSession]
        buf.extend_from_slice(&pack_u16_array(&[0x1001, 0x1002, 0x1003]));
        // EventsSupported: [ObjectAdded, ObjectRemoved]
        buf.extend_from_slice(&pack_u16_array(&[0x4002, 0x4003]));
        // DevicePropertiesSupported: [0x5001, 0x5002]
        buf.extend_from_slice(&pack_u16_array(&[0x5001, 0x5002]));
        // CaptureFormats: [JPEG]
        buf.extend_from_slice(&pack_u16_array(&[0x3801]));
        // PlaybackFormats: [JPEG, MP3]
        buf.extend_from_slice(&pack_u16_array(&[0x3801, 0x3009]));
        // Manufacturer: "Test Manufacturer"
        buf.extend_from_slice(&pack_string("Test Manufacturer"));
        // Model: "Test Model"
        buf.extend_from_slice(&pack_string("Test Model"));
        // DeviceVersion: "1.0.0"
        buf.extend_from_slice(&pack_string("1.0.0"));
        // SerialNumber: "ABC123"
        buf.extend_from_slice(&pack_string("ABC123"));

        buf
    }

    #[test]
    fn device_info_parse_full() {
        let buf = build_full_device_info_bytes();
        let info = DeviceInfo::from_bytes(&buf).unwrap();

        assert_eq!(info.standard_version, 100);
        assert_eq!(info.vendor_extension_id, 6);
        assert_eq!(info.vendor_extension_version, 100);
        assert_eq!(info.vendor_extension_desc, "microsoft.com: 1.0");
        assert_eq!(info.functional_mode, 0);

        assert_eq!(info.operations_supported.len(), 3);
        assert_eq!(info.operations_supported[0], OperationCode::GetDeviceInfo);
        assert_eq!(info.operations_supported[1], OperationCode::OpenSession);
        assert_eq!(info.operations_supported[2], OperationCode::CloseSession);

        assert_eq!(info.events_supported.len(), 2);
        assert_eq!(info.events_supported[0], EventCode::ObjectAdded);
        assert_eq!(info.events_supported[1], EventCode::ObjectRemoved);

        assert_eq!(info.device_properties_supported, vec![0x5001, 0x5002]);

        assert_eq!(info.capture_formats.len(), 1);
        assert_eq!(info.capture_formats[0], ObjectFormatCode::Jpeg);

        assert_eq!(info.playback_formats.len(), 2);
        assert_eq!(info.playback_formats[0], ObjectFormatCode::Jpeg);
        assert_eq!(info.playback_formats[1], ObjectFormatCode::Mp3);

        assert_eq!(info.manufacturer, "Test Manufacturer");
        assert_eq!(info.model, "Test Model");
        assert_eq!(info.device_version, "1.0.0");
        assert_eq!(info.serial_number, "ABC123");
    }

    #[test]
    fn device_info_parse_insufficient_bytes() {
        let buf = vec![0x00, 0x01]; // Only 2 bytes
        assert!(DeviceInfo::from_bytes(&buf).is_err());
    }

    // =========================================================================
    // StorageInfo Tests
    // =========================================================================

    fn build_storage_info_bytes() -> Vec<u8> {
        let mut buf = Vec::new();

        // StorageType: RemovableRam (4)
        buf.extend_from_slice(&pack_u16(4));
        // FilesystemType: GenericHierarchical (2)
        buf.extend_from_slice(&pack_u16(2));
        // AccessCapability: ReadWrite (0)
        buf.extend_from_slice(&pack_u16(0));
        // MaxCapacity: 32GB
        buf.extend_from_slice(&32_000_000_000u64.to_le_bytes());
        // FreeSpaceInBytes: 16GB
        buf.extend_from_slice(&16_000_000_000u64.to_le_bytes());
        // FreeSpaceInObjects: 0xFFFFFFFF (unknown)
        buf.extend_from_slice(&pack_u32(0xFFFFFFFF));
        // StorageDescription: "SD Card"
        buf.extend_from_slice(&pack_string("SD Card"));
        // VolumeIdentifier: "VOL001"
        buf.extend_from_slice(&pack_string("VOL001"));

        buf
    }

    #[test]
    fn storage_info_parse() {
        let buf = build_storage_info_bytes();
        let info = StorageInfo::from_bytes(&buf).unwrap();

        assert_eq!(info.storage_type, StorageType::RemovableRam);
        assert_eq!(info.filesystem_type, FilesystemType::GenericHierarchical);
        assert_eq!(info.access_capability, AccessCapability::ReadWrite);
        assert_eq!(info.max_capacity, 32_000_000_000);
        assert_eq!(info.free_space_bytes, 16_000_000_000);
        assert_eq!(info.free_space_objects, 0xFFFFFFFF);
        assert_eq!(info.description, "SD Card");
        assert_eq!(info.volume_identifier, "VOL001");
    }

    #[test]
    fn storage_info_parse_insufficient_bytes() {
        let buf = vec![0x00; 10]; // Not enough bytes
        assert!(StorageInfo::from_bytes(&buf).is_err());
    }

    // =========================================================================
    // ObjectInfo Tests
    // =========================================================================

    fn build_file_object_info_bytes() -> Vec<u8> {
        let mut buf = Vec::new();

        // StorageID: 0x00010001
        buf.extend_from_slice(&pack_u32(0x00010001));
        // ObjectFormat: JPEG (0x3801)
        buf.extend_from_slice(&pack_u16(0x3801));
        // ProtectionStatus: None (0)
        buf.extend_from_slice(&pack_u16(0));
        // ObjectCompressedSize: 1024 bytes
        buf.extend_from_slice(&pack_u32(1024));
        // ThumbFormat: JPEG (0x3801)
        buf.extend_from_slice(&pack_u16(0x3801));
        // ThumbCompressedSize: 512
        buf.extend_from_slice(&pack_u32(512));
        // ThumbPixWidth: 160
        buf.extend_from_slice(&pack_u32(160));
        // ThumbPixHeight: 120
        buf.extend_from_slice(&pack_u32(120));
        // ImagePixWidth: 1920
        buf.extend_from_slice(&pack_u32(1920));
        // ImagePixHeight: 1080
        buf.extend_from_slice(&pack_u32(1080));
        // ImageBitDepth: 24
        buf.extend_from_slice(&pack_u32(24));
        // ParentObject: 0x00000005
        buf.extend_from_slice(&pack_u32(5));
        // AssociationType: None (0)
        buf.extend_from_slice(&pack_u16(0));
        // AssociationDesc: 0
        buf.extend_from_slice(&pack_u32(0));
        // SequenceNumber: 1
        buf.extend_from_slice(&pack_u32(1));
        // Filename: "photo.jpg"
        buf.extend_from_slice(&pack_string("photo.jpg"));
        // DateCreated: "20240315T143022"
        buf.extend_from_slice(&pack_datetime(&DateTime {
            year: 2024,
            month: 3,
            day: 15,
            hour: 14,
            minute: 30,
            second: 22,
        }));
        // DateModified: "20240316T090000"
        buf.extend_from_slice(&pack_datetime(&DateTime {
            year: 2024,
            month: 3,
            day: 16,
            hour: 9,
            minute: 0,
            second: 0,
        }));
        // Keywords: ""
        buf.push(0x00);

        buf
    }

    #[test]
    fn object_info_parse_file() {
        let buf = build_file_object_info_bytes();
        let info = ObjectInfo::from_bytes(&buf).unwrap();

        assert_eq!(info.storage_id, StorageId(0x00010001));
        assert_eq!(info.format, ObjectFormatCode::Jpeg);
        assert_eq!(info.protection_status, ProtectionStatus::None);
        assert_eq!(info.size, 1024);
        assert_eq!(info.thumb_format, ObjectFormatCode::Jpeg);
        assert_eq!(info.thumb_size, 512);
        assert_eq!(info.thumb_width, 160);
        assert_eq!(info.thumb_height, 120);
        assert_eq!(info.image_width, 1920);
        assert_eq!(info.image_height, 1080);
        assert_eq!(info.image_bit_depth, 24);
        assert_eq!(info.parent, ObjectHandle(5));
        assert_eq!(info.association_type, AssociationType::None);
        assert_eq!(info.association_desc, 0);
        assert_eq!(info.sequence_number, 1);
        assert_eq!(info.filename, "photo.jpg");
        assert!(info.created.is_some());
        let created = info.created.unwrap();
        assert_eq!(created.year, 2024);
        assert_eq!(created.month, 3);
        assert_eq!(created.day, 15);
        assert!(info.modified.is_some());
        assert_eq!(info.keywords, "");

        assert!(info.is_file());
        assert!(!info.is_folder());
    }

    fn build_folder_object_info_bytes() -> Vec<u8> {
        let mut buf = Vec::new();

        // StorageID: 0x00010001
        buf.extend_from_slice(&pack_u32(0x00010001));
        // ObjectFormat: Association (0x3001)
        buf.extend_from_slice(&pack_u16(0x3001));
        // ProtectionStatus: None (0)
        buf.extend_from_slice(&pack_u16(0));
        // ObjectCompressedSize: 0
        buf.extend_from_slice(&pack_u32(0));
        // ThumbFormat: Undefined (0x3000)
        buf.extend_from_slice(&pack_u16(0x3000));
        // ThumbCompressedSize: 0
        buf.extend_from_slice(&pack_u32(0));
        // ThumbPixWidth: 0
        buf.extend_from_slice(&pack_u32(0));
        // ThumbPixHeight: 0
        buf.extend_from_slice(&pack_u32(0));
        // ImagePixWidth: 0
        buf.extend_from_slice(&pack_u32(0));
        // ImagePixHeight: 0
        buf.extend_from_slice(&pack_u32(0));
        // ImageBitDepth: 0
        buf.extend_from_slice(&pack_u32(0));
        // ParentObject: ROOT (0)
        buf.extend_from_slice(&pack_u32(0));
        // AssociationType: GenericFolder (1)
        buf.extend_from_slice(&pack_u16(1));
        // AssociationDesc: 0
        buf.extend_from_slice(&pack_u32(0));
        // SequenceNumber: 0
        buf.extend_from_slice(&pack_u32(0));
        // Filename: "DCIM"
        buf.extend_from_slice(&pack_string("DCIM"));
        // DateCreated: empty
        buf.push(0x00);
        // DateModified: empty
        buf.push(0x00);
        // Keywords: ""
        buf.push(0x00);

        buf
    }

    #[test]
    fn object_info_parse_folder() {
        let buf = build_folder_object_info_bytes();
        let info = ObjectInfo::from_bytes(&buf).unwrap();

        assert_eq!(info.format, ObjectFormatCode::Association);
        assert_eq!(info.association_type, AssociationType::GenericFolder);
        assert_eq!(info.filename, "DCIM");
        assert_eq!(info.parent, ObjectHandle::ROOT);
        assert!(info.created.is_none());
        assert!(info.modified.is_none());

        assert!(info.is_folder());
        assert!(!info.is_file());
    }

    #[test]
    fn object_info_to_bytes_roundtrip() {
        let original = ObjectInfo {
            storage_id: StorageId(0x00010001),
            format: ObjectFormatCode::Jpeg,
            protection_status: ProtectionStatus::None,
            size: 2048,
            thumb_format: ObjectFormatCode::Jpeg,
            thumb_size: 256,
            thumb_width: 80,
            thumb_height: 60,
            image_width: 800,
            image_height: 600,
            image_bit_depth: 24,
            parent: ObjectHandle(10),
            association_type: AssociationType::None,
            association_desc: 0,
            sequence_number: 5,
            filename: "test.jpg".to_string(),
            created: Some(DateTime {
                year: 2024,
                month: 6,
                day: 15,
                hour: 10,
                minute: 30,
                second: 0,
            }),
            modified: Some(DateTime {
                year: 2024,
                month: 6,
                day: 16,
                hour: 11,
                minute: 45,
                second: 30,
            }),
            keywords: "test,photo".to_string(),
        };

        let bytes = original.to_bytes();
        let parsed = ObjectInfo::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.storage_id, original.storage_id);
        assert_eq!(parsed.format, original.format);
        assert_eq!(parsed.protection_status, original.protection_status);
        assert_eq!(parsed.size, original.size);
        assert_eq!(parsed.thumb_format, original.thumb_format);
        assert_eq!(parsed.thumb_size, original.thumb_size);
        assert_eq!(parsed.thumb_width, original.thumb_width);
        assert_eq!(parsed.thumb_height, original.thumb_height);
        assert_eq!(parsed.image_width, original.image_width);
        assert_eq!(parsed.image_height, original.image_height);
        assert_eq!(parsed.image_bit_depth, original.image_bit_depth);
        assert_eq!(parsed.parent, original.parent);
        assert_eq!(parsed.association_type, original.association_type);
        assert_eq!(parsed.association_desc, original.association_desc);
        assert_eq!(parsed.sequence_number, original.sequence_number);
        assert_eq!(parsed.filename, original.filename);
        assert_eq!(parsed.created, original.created);
        assert_eq!(parsed.modified, original.modified);
        assert_eq!(parsed.keywords, original.keywords);
    }

    #[test]
    fn object_info_to_bytes_large_size() {
        let info = ObjectInfo {
            size: 5_000_000_000, // 5GB, larger than u32::MAX
            ..Default::default()
        };

        let bytes = info.to_bytes();
        let parsed = ObjectInfo::from_bytes(&bytes).unwrap();

        // Should be capped at u32::MAX when serializing
        assert_eq!(parsed.size, u32::MAX as u64);
    }

    #[test]
    fn object_info_is_folder_by_format() {
        let info = ObjectInfo {
            format: ObjectFormatCode::Association,
            association_type: AssociationType::None,
            ..Default::default()
        };
        assert!(info.is_folder());
    }

    #[test]
    fn object_info_is_folder_by_association() {
        let info = ObjectInfo {
            format: ObjectFormatCode::Undefined,
            association_type: AssociationType::GenericFolder,
            ..Default::default()
        };
        assert!(info.is_folder());
    }

    #[test]
    fn object_info_is_file() {
        let info = ObjectInfo {
            format: ObjectFormatCode::Jpeg,
            association_type: AssociationType::None,
            ..Default::default()
        };
        assert!(info.is_file());
        assert!(!info.is_folder());
    }

    #[test]
    fn object_info_parse_insufficient_bytes() {
        let buf = vec![0x00; 10]; // Not enough bytes
        assert!(ObjectInfo::from_bytes(&buf).is_err());
    }

    #[test]
    fn object_info_default() {
        let info = ObjectInfo::default();
        assert_eq!(info.storage_id, StorageId::default());
        assert_eq!(info.format, ObjectFormatCode::Undefined);
        assert_eq!(info.protection_status, ProtectionStatus::None);
        assert_eq!(info.size, 0);
        assert_eq!(info.filename, "");
        assert!(info.created.is_none());
        assert!(info.modified.is_none());
    }
}
