//! MTP/PTP data structures for device, storage, and object information.
//!
//! This module provides high-level structures for parsing protocol responses:
//! - [`DeviceInfo`]: Device capabilities and identification
//! - [`StorageInfo`]: Storage characteristics and capacity
//! - [`ObjectInfo`]: File/folder metadata
//! - [`DevicePropDesc`]: Device property descriptors
//! - [`PropertyValue`]: Property values of various types

use super::codes::{DevicePropertyCode, PropertyDataType};
use super::pack::{
    pack_datetime, pack_i16, pack_i32, pack_i64, pack_i8, pack_string, pack_u16, pack_u32,
    pack_u64, pack_u8, unpack_datetime, unpack_i16, unpack_i32, unpack_i64, unpack_i8,
    unpack_string, unpack_u16, unpack_u16_array, unpack_u32, unpack_u64, unpack_u8, DateTime,
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
// PropertyValue Enum
// =============================================================================

/// A property value with its associated type.
///
/// Used for device property values in `DevicePropDesc`, as well as
/// for get/set device property operations.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// Signed 8-bit integer.
    Int8(i8),
    /// Unsigned 8-bit integer.
    Uint8(u8),
    /// Signed 16-bit integer.
    Int16(i16),
    /// Unsigned 16-bit integer.
    Uint16(u16),
    /// Signed 32-bit integer.
    Int32(i32),
    /// Unsigned 32-bit integer.
    Uint32(u32),
    /// Signed 64-bit integer.
    Int64(i64),
    /// Unsigned 64-bit integer.
    Uint64(u64),
    /// UTF-16LE encoded string.
    String(String),
}

impl PropertyValue {
    /// Serialize this property value to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            PropertyValue::Int8(v) => pack_i8(*v).to_vec(),
            PropertyValue::Uint8(v) => pack_u8(*v).to_vec(),
            PropertyValue::Int16(v) => pack_i16(*v).to_vec(),
            PropertyValue::Uint16(v) => pack_u16(*v).to_vec(),
            PropertyValue::Int32(v) => pack_i32(*v).to_vec(),
            PropertyValue::Uint32(v) => pack_u32(*v).to_vec(),
            PropertyValue::Int64(v) => pack_i64(*v).to_vec(),
            PropertyValue::Uint64(v) => pack_u64(*v).to_vec(),
            PropertyValue::String(v) => pack_string(v),
        }
    }

    /// Parse a property value from bytes given the expected data type.
    ///
    /// Returns the parsed value and the number of bytes consumed.
    pub fn from_bytes(
        buf: &[u8],
        data_type: PropertyDataType,
    ) -> Result<(Self, usize), crate::Error> {
        match data_type {
            PropertyDataType::Int8 => {
                let val = unpack_i8(buf)?;
                Ok((PropertyValue::Int8(val), 1))
            }
            PropertyDataType::Uint8 => {
                let val = unpack_u8(buf)?;
                Ok((PropertyValue::Uint8(val), 1))
            }
            PropertyDataType::Int16 => {
                let val = unpack_i16(buf)?;
                Ok((PropertyValue::Int16(val), 2))
            }
            PropertyDataType::Uint16 => {
                let val = unpack_u16(buf)?;
                Ok((PropertyValue::Uint16(val), 2))
            }
            PropertyDataType::Int32 => {
                let val = unpack_i32(buf)?;
                Ok((PropertyValue::Int32(val), 4))
            }
            PropertyDataType::Uint32 => {
                let val = unpack_u32(buf)?;
                Ok((PropertyValue::Uint32(val), 4))
            }
            PropertyDataType::Int64 => {
                let val = unpack_i64(buf)?;
                Ok((PropertyValue::Int64(val), 8))
            }
            PropertyDataType::Uint64 => {
                let val = unpack_u64(buf)?;
                Ok((PropertyValue::Uint64(val), 8))
            }
            PropertyDataType::String => {
                let (val, consumed) = unpack_string(buf)?;
                Ok((PropertyValue::String(val), consumed))
            }
            PropertyDataType::Undefined
            | PropertyDataType::Int128
            | PropertyDataType::Uint128
            | PropertyDataType::Unknown(_) => Err(crate::Error::invalid_data(format!(
                "unsupported property data type: {:?}",
                data_type
            ))),
        }
    }

    /// Get the data type of this property value.
    pub fn data_type(&self) -> PropertyDataType {
        match self {
            PropertyValue::Int8(_) => PropertyDataType::Int8,
            PropertyValue::Uint8(_) => PropertyDataType::Uint8,
            PropertyValue::Int16(_) => PropertyDataType::Int16,
            PropertyValue::Uint16(_) => PropertyDataType::Uint16,
            PropertyValue::Int32(_) => PropertyDataType::Int32,
            PropertyValue::Uint32(_) => PropertyDataType::Uint32,
            PropertyValue::Int64(_) => PropertyDataType::Int64,
            PropertyValue::Uint64(_) => PropertyDataType::Uint64,
            PropertyValue::String(_) => PropertyDataType::String,
        }
    }
}

// =============================================================================
// PropertyFormType Enum
// =============================================================================

/// Form type for property value constraints.
///
/// Describes how allowed values are specified for a property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PropertyFormType {
    /// No constraints (any value valid).
    #[default]
    None,
    /// Value must be within a range (min, max, step).
    Range,
    /// Value must be one of an enumerated set.
    Enumeration,
    /// Unknown form type.
    Unknown(u8),
}

impl PropertyFormType {
    /// Convert a raw u8 code to a PropertyFormType.
    pub fn from_code(code: u8) -> Self {
        match code {
            0x00 => PropertyFormType::None,
            0x01 => PropertyFormType::Range,
            0x02 => PropertyFormType::Enumeration,
            _ => PropertyFormType::Unknown(code),
        }
    }

    /// Convert a PropertyFormType to its raw u8 value.
    pub fn to_code(self) -> u8 {
        match self {
            PropertyFormType::None => 0x00,
            PropertyFormType::Range => 0x01,
            PropertyFormType::Enumeration => 0x02,
            PropertyFormType::Unknown(code) => code,
        }
    }
}

// =============================================================================
// PropertyRange Struct
// =============================================================================

/// Range constraint for a property value.
///
/// Used when `PropertyFormType::Range` is specified.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyRange {
    /// Minimum allowed value.
    pub min: PropertyValue,
    /// Maximum allowed value.
    pub max: PropertyValue,
    /// Step size between allowed values.
    pub step: PropertyValue,
}

impl PropertyRange {
    /// Parse a PropertyRange from bytes given the data type.
    ///
    /// Returns the parsed range and the number of bytes consumed.
    pub fn from_bytes(
        buf: &[u8],
        data_type: PropertyDataType,
    ) -> Result<(Self, usize), crate::Error> {
        let mut offset = 0;

        let (min, consumed) = PropertyValue::from_bytes(&buf[offset..], data_type)?;
        offset += consumed;

        let (max, consumed) = PropertyValue::from_bytes(&buf[offset..], data_type)?;
        offset += consumed;

        let (step, consumed) = PropertyValue::from_bytes(&buf[offset..], data_type)?;
        offset += consumed;

        Ok((PropertyRange { min, max, step }, offset))
    }

    /// Serialize this property range to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.min.to_bytes());
        buf.extend_from_slice(&self.max.to_bytes());
        buf.extend_from_slice(&self.step.to_bytes());
        buf
    }
}

// =============================================================================
// DevicePropDesc Structure
// =============================================================================

/// Device property descriptor.
///
/// Describes a device property including its type, current value,
/// default value, and allowed values/ranges.
///
/// Returned by the GetDevicePropDesc operation.
#[derive(Debug, Clone)]
pub struct DevicePropDesc {
    /// Property code identifying this property.
    pub property_code: DevicePropertyCode,
    /// Data type of the property value.
    pub data_type: PropertyDataType,
    /// Whether the property is writable (true) or read-only (false).
    pub writable: bool,
    /// Default/factory value.
    pub default_value: PropertyValue,
    /// Current value.
    pub current_value: PropertyValue,
    /// Form type (None, Range, or Enumeration).
    pub form_type: PropertyFormType,
    /// Allowed values (if form_type is Enumeration).
    pub enum_values: Option<Vec<PropertyValue>>,
    /// Value range (if form_type is Range).
    pub range: Option<PropertyRange>,
}

impl DevicePropDesc {
    /// Parse a DevicePropDesc from bytes.
    ///
    /// The buffer should contain the DevicePropDesc dataset as returned
    /// by GetDevicePropDesc.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::Error> {
        let mut offset = 0;

        // 1. PropertyCode (u16)
        let property_code = DevicePropertyCode::from_code(unpack_u16(&buf[offset..])?);
        offset += 2;

        // 2. DataType (u16)
        let data_type = PropertyDataType::from_code(unpack_u16(&buf[offset..])?);
        offset += 2;

        // 3. GetSet (u8): 0x00 = read-only, 0x01 = read-write
        if buf.len() <= offset {
            return Err(crate::Error::invalid_data(
                "DevicePropDesc: insufficient bytes for GetSet",
            ));
        }
        let writable = buf[offset] != 0x00;
        offset += 1;

        // 4. DefaultValue (variable size based on data type)
        let (default_value, consumed) = PropertyValue::from_bytes(&buf[offset..], data_type)?;
        offset += consumed;

        // 5. CurrentValue (variable size based on data type)
        let (current_value, consumed) = PropertyValue::from_bytes(&buf[offset..], data_type)?;
        offset += consumed;

        // 6. FormFlag (u8)
        if buf.len() <= offset {
            return Err(crate::Error::invalid_data(
                "DevicePropDesc: insufficient bytes for FormFlag",
            ));
        }
        let form_type = PropertyFormType::from_code(buf[offset]);
        offset += 1;

        // 7. Form data (depends on form_type)
        let (enum_values, range) = match form_type {
            PropertyFormType::None | PropertyFormType::Unknown(_) => (None, None),
            PropertyFormType::Range => {
                let (range, _consumed) = PropertyRange::from_bytes(&buf[offset..], data_type)?;
                (None, Some(range))
            }
            PropertyFormType::Enumeration => {
                // Number of values (u16)
                let count = unpack_u16(&buf[offset..])? as usize;
                offset += 2;

                let mut values = Vec::with_capacity(count);
                for _ in 0..count {
                    let (val, consumed) = PropertyValue::from_bytes(&buf[offset..], data_type)?;
                    values.push(val);
                    offset += consumed;
                }
                (Some(values), None)
            }
        };

        Ok(DevicePropDesc {
            property_code,
            data_type,
            writable,
            default_value,
            current_value,
            form_type,
            enum_values,
            range,
        })
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

    /// Check if the device supports a specific operation.
    ///
    /// # Arguments
    ///
    /// * `operation` - The operation code to check
    ///
    /// # Returns
    ///
    /// Returns true if the operation is in the device's supported operations list.
    pub fn supports_operation(&self, operation: OperationCode) -> bool {
        self.operations_supported.contains(&operation)
    }

    /// Check if the device supports renaming objects.
    ///
    /// This checks for support of the SetObjectPropValue operation (0x9804),
    /// which is required to rename files and folders via the ObjectFileName property.
    ///
    /// # Returns
    ///
    /// Returns true if the device advertises SetObjectPropValue support.
    pub fn supports_rename(&self) -> bool {
        self.supports_operation(OperationCode::SetObjectPropValue)
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
    /// Object handle (set after parsing, not part of protocol data).
    pub handle: ObjectHandle,
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
            handle: ObjectHandle::default(), // Set by caller after parsing
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
            handle: ObjectHandle(42),
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

    // =========================================================================
    // DeviceInfo capability tests
    // =========================================================================

    #[test]
    fn device_info_supports_operation() {
        let info = DeviceInfo {
            operations_supported: vec![
                OperationCode::GetDeviceInfo,
                OperationCode::OpenSession,
                OperationCode::SetObjectPropValue,
            ],
            ..Default::default()
        };

        assert!(info.supports_operation(OperationCode::GetDeviceInfo));
        assert!(info.supports_operation(OperationCode::OpenSession));
        assert!(info.supports_operation(OperationCode::SetObjectPropValue));
        assert!(!info.supports_operation(OperationCode::DeleteObject));
        assert!(!info.supports_operation(OperationCode::GetObjectPropValue));
    }

    #[test]
    fn device_info_supports_rename_true() {
        let info = DeviceInfo {
            operations_supported: vec![
                OperationCode::GetDeviceInfo,
                OperationCode::SetObjectPropValue, // Required for rename
            ],
            ..Default::default()
        };

        assert!(info.supports_rename());
    }

    #[test]
    fn device_info_supports_rename_false() {
        let info = DeviceInfo {
            operations_supported: vec![
                OperationCode::GetDeviceInfo,
                OperationCode::GetObjectPropValue, // Has Get but not Set
            ],
            ..Default::default()
        };

        assert!(!info.supports_rename());
    }

    #[test]
    fn device_info_supports_rename_empty() {
        let info = DeviceInfo::default();
        assert!(!info.supports_rename());
    }

    // =========================================================================
    // PropertyValue Tests
    // =========================================================================

    #[test]
    fn property_value_to_bytes_int8() {
        assert_eq!(PropertyValue::Int8(0).to_bytes(), vec![0x00]);
        assert_eq!(PropertyValue::Int8(127).to_bytes(), vec![0x7F]);
        assert_eq!(PropertyValue::Int8(-1).to_bytes(), vec![0xFF]);
        assert_eq!(PropertyValue::Int8(-128).to_bytes(), vec![0x80]);
    }

    #[test]
    fn property_value_to_bytes_uint8() {
        assert_eq!(PropertyValue::Uint8(0).to_bytes(), vec![0x00]);
        assert_eq!(PropertyValue::Uint8(255).to_bytes(), vec![0xFF]);
    }

    #[test]
    fn property_value_to_bytes_int16() {
        assert_eq!(PropertyValue::Int16(0).to_bytes(), vec![0x00, 0x00]);
        assert_eq!(PropertyValue::Int16(-1).to_bytes(), vec![0xFF, 0xFF]);
        assert_eq!(PropertyValue::Int16(0x1234).to_bytes(), vec![0x34, 0x12]);
    }

    #[test]
    fn property_value_to_bytes_uint16() {
        assert_eq!(PropertyValue::Uint16(0).to_bytes(), vec![0x00, 0x00]);
        assert_eq!(PropertyValue::Uint16(0x1234).to_bytes(), vec![0x34, 0x12]);
    }

    #[test]
    fn property_value_to_bytes_int32() {
        assert_eq!(
            PropertyValue::Int32(0x12345678).to_bytes(),
            vec![0x78, 0x56, 0x34, 0x12]
        );
        assert_eq!(
            PropertyValue::Int32(-1).to_bytes(),
            vec![0xFF, 0xFF, 0xFF, 0xFF]
        );
    }

    #[test]
    fn property_value_to_bytes_uint32() {
        assert_eq!(
            PropertyValue::Uint32(0x12345678).to_bytes(),
            vec![0x78, 0x56, 0x34, 0x12]
        );
    }

    #[test]
    fn property_value_to_bytes_string() {
        // Empty string
        assert_eq!(PropertyValue::String("".to_string()).to_bytes(), vec![0x00]);
        // Non-empty string
        let bytes = PropertyValue::String("Hi".to_string()).to_bytes();
        assert_eq!(bytes[0], 3); // length including null
    }

    #[test]
    fn property_value_from_bytes_int8() {
        let (val, consumed) = PropertyValue::from_bytes(&[0x80], PropertyDataType::Int8).unwrap();
        assert_eq!(val, PropertyValue::Int8(-128));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn property_value_from_bytes_uint8() {
        let (val, consumed) = PropertyValue::from_bytes(&[0x64], PropertyDataType::Uint8).unwrap();
        assert_eq!(val, PropertyValue::Uint8(100));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn property_value_from_bytes_int16() {
        let (val, consumed) =
            PropertyValue::from_bytes(&[0xFE, 0xFF], PropertyDataType::Int16).unwrap();
        assert_eq!(val, PropertyValue::Int16(-2));
        assert_eq!(consumed, 2);
    }

    #[test]
    fn property_value_from_bytes_uint16() {
        let (val, consumed) =
            PropertyValue::from_bytes(&[0x90, 0x01], PropertyDataType::Uint16).unwrap();
        assert_eq!(val, PropertyValue::Uint16(400));
        assert_eq!(consumed, 2);
    }

    #[test]
    fn property_value_from_bytes_int32() {
        let (val, consumed) =
            PropertyValue::from_bytes(&[0xFF, 0xFF, 0xFF, 0xFF], PropertyDataType::Int32).unwrap();
        assert_eq!(val, PropertyValue::Int32(-1));
        assert_eq!(consumed, 4);
    }

    #[test]
    fn property_value_from_bytes_uint32() {
        let (val, consumed) =
            PropertyValue::from_bytes(&[0x78, 0x56, 0x34, 0x12], PropertyDataType::Uint32).unwrap();
        assert_eq!(val, PropertyValue::Uint32(0x12345678));
        assert_eq!(consumed, 4);
    }

    #[test]
    fn property_value_from_bytes_int64() {
        let (val, consumed) = PropertyValue::from_bytes(
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            PropertyDataType::Int64,
        )
        .unwrap();
        assert_eq!(val, PropertyValue::Int64(-1));
        assert_eq!(consumed, 8);
    }

    #[test]
    fn property_value_from_bytes_uint64() {
        let (val, consumed) = PropertyValue::from_bytes(
            &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01],
            PropertyDataType::Uint64,
        )
        .unwrap();
        assert_eq!(val, PropertyValue::Uint64(0x0102030405060708));
        assert_eq!(consumed, 8);
    }

    #[test]
    fn property_value_from_bytes_string() {
        let buf = vec![
            0x03, // length = 3
            0x48, 0x00, // 'H'
            0x69, 0x00, // 'i'
            0x00, 0x00, // null
        ];
        let (val, consumed) = PropertyValue::from_bytes(&buf, PropertyDataType::String).unwrap();
        assert_eq!(val, PropertyValue::String("Hi".to_string()));
        assert_eq!(consumed, 7);
    }

    #[test]
    fn property_value_roundtrip() {
        let values = [
            PropertyValue::Int8(-42),
            PropertyValue::Uint8(100),
            PropertyValue::Int16(-1000),
            PropertyValue::Uint16(5000),
            PropertyValue::Int32(-100000),
            PropertyValue::Uint32(100000),
            PropertyValue::Int64(-1_000_000_000),
            PropertyValue::Uint64(1_000_000_000),
            PropertyValue::String("Test".to_string()),
        ];

        for val in &values {
            let bytes = val.to_bytes();
            let (parsed, _) = PropertyValue::from_bytes(&bytes, val.data_type()).unwrap();
            assert_eq!(&parsed, val);
        }
    }

    #[test]
    fn property_value_data_type() {
        assert_eq!(PropertyValue::Int8(0).data_type(), PropertyDataType::Int8);
        assert_eq!(PropertyValue::Uint8(0).data_type(), PropertyDataType::Uint8);
        assert_eq!(PropertyValue::Int16(0).data_type(), PropertyDataType::Int16);
        assert_eq!(
            PropertyValue::Uint16(0).data_type(),
            PropertyDataType::Uint16
        );
        assert_eq!(PropertyValue::Int32(0).data_type(), PropertyDataType::Int32);
        assert_eq!(
            PropertyValue::Uint32(0).data_type(),
            PropertyDataType::Uint32
        );
        assert_eq!(PropertyValue::Int64(0).data_type(), PropertyDataType::Int64);
        assert_eq!(
            PropertyValue::Uint64(0).data_type(),
            PropertyDataType::Uint64
        );
        assert_eq!(
            PropertyValue::String("".to_string()).data_type(),
            PropertyDataType::String
        );
    }

    #[test]
    fn property_value_from_bytes_unsupported_type() {
        assert!(PropertyValue::from_bytes(&[0x00], PropertyDataType::Undefined).is_err());
        assert!(PropertyValue::from_bytes(&[0x00], PropertyDataType::Int128).is_err());
        assert!(PropertyValue::from_bytes(&[0x00], PropertyDataType::Uint128).is_err());
        assert!(PropertyValue::from_bytes(&[0x00], PropertyDataType::Unknown(0x99)).is_err());
    }

    #[test]
    fn property_value_from_bytes_insufficient_bytes() {
        assert!(PropertyValue::from_bytes(&[], PropertyDataType::Int8).is_err());
        assert!(PropertyValue::from_bytes(&[0x00], PropertyDataType::Int16).is_err());
        assert!(PropertyValue::from_bytes(&[0x00, 0x00], PropertyDataType::Int32).is_err());
        assert!(PropertyValue::from_bytes(&[0x00; 7], PropertyDataType::Int64).is_err());
    }

    // =========================================================================
    // PropertyFormType Tests
    // =========================================================================

    #[test]
    fn property_form_type_from_code() {
        assert_eq!(PropertyFormType::from_code(0x00), PropertyFormType::None);
        assert_eq!(PropertyFormType::from_code(0x01), PropertyFormType::Range);
        assert_eq!(
            PropertyFormType::from_code(0x02),
            PropertyFormType::Enumeration
        );
        assert_eq!(
            PropertyFormType::from_code(0x99),
            PropertyFormType::Unknown(0x99)
        );
    }

    #[test]
    fn property_form_type_to_code() {
        assert_eq!(PropertyFormType::None.to_code(), 0x00);
        assert_eq!(PropertyFormType::Range.to_code(), 0x01);
        assert_eq!(PropertyFormType::Enumeration.to_code(), 0x02);
        assert_eq!(PropertyFormType::Unknown(0x99).to_code(), 0x99);
    }

    #[test]
    fn property_form_type_roundtrip() {
        let forms = [
            PropertyFormType::None,
            PropertyFormType::Range,
            PropertyFormType::Enumeration,
        ];
        for f in forms {
            assert_eq!(PropertyFormType::from_code(f.to_code()), f);
        }
    }

    // =========================================================================
    // PropertyRange Tests
    // =========================================================================

    #[test]
    fn property_range_from_bytes_uint8() {
        // Range: min=0, max=100, step=1
        let buf = vec![0x00, 0x64, 0x01];
        let (range, consumed) = PropertyRange::from_bytes(&buf, PropertyDataType::Uint8).unwrap();
        assert_eq!(range.min, PropertyValue::Uint8(0));
        assert_eq!(range.max, PropertyValue::Uint8(100));
        assert_eq!(range.step, PropertyValue::Uint8(1));
        assert_eq!(consumed, 3);
    }

    #[test]
    fn property_range_from_bytes_uint16() {
        // Range: min=100 (0x0064), max=6400 (0x1900), step=100 (0x0064)
        let buf = vec![0x64, 0x00, 0x00, 0x19, 0x64, 0x00];
        let (range, consumed) = PropertyRange::from_bytes(&buf, PropertyDataType::Uint16).unwrap();
        assert_eq!(range.min, PropertyValue::Uint16(100));
        assert_eq!(range.max, PropertyValue::Uint16(6400));
        assert_eq!(range.step, PropertyValue::Uint16(100));
        assert_eq!(consumed, 6);
    }

    #[test]
    fn property_range_to_bytes() {
        let range = PropertyRange {
            min: PropertyValue::Uint8(0),
            max: PropertyValue::Uint8(100),
            step: PropertyValue::Uint8(1),
        };
        assert_eq!(range.to_bytes(), vec![0x00, 0x64, 0x01]);
    }

    #[test]
    fn property_range_roundtrip() {
        let range = PropertyRange {
            min: PropertyValue::Uint16(100),
            max: PropertyValue::Uint16(6400),
            step: PropertyValue::Uint16(100),
        };
        let bytes = range.to_bytes();
        let (parsed, _) = PropertyRange::from_bytes(&bytes, PropertyDataType::Uint16).unwrap();
        assert_eq!(parsed.min, range.min);
        assert_eq!(parsed.max, range.max);
        assert_eq!(parsed.step, range.step);
    }

    // =========================================================================
    // DevicePropDesc Tests
    // =========================================================================

    /// Build a BatteryLevel property descriptor bytes for testing.
    fn build_battery_level_prop_desc(current: u8) -> Vec<u8> {
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

    #[test]
    fn device_prop_desc_parse_battery_level() {
        let buf = build_battery_level_prop_desc(75);
        let desc = DevicePropDesc::from_bytes(&buf).unwrap();

        assert_eq!(desc.property_code, DevicePropertyCode::BatteryLevel);
        assert_eq!(desc.data_type, PropertyDataType::Uint8);
        assert!(!desc.writable);
        assert_eq!(desc.default_value, PropertyValue::Uint8(100));
        assert_eq!(desc.current_value, PropertyValue::Uint8(75));
        assert_eq!(desc.form_type, PropertyFormType::Range);
        assert!(desc.enum_values.is_none());
        assert!(desc.range.is_some());

        let range = desc.range.unwrap();
        assert_eq!(range.min, PropertyValue::Uint8(0));
        assert_eq!(range.max, PropertyValue::Uint8(100));
        assert_eq!(range.step, PropertyValue::Uint8(1));
    }

    /// Build an ISO property descriptor with enumeration form.
    fn build_iso_prop_desc() -> Vec<u8> {
        let mut buf = Vec::new();
        // PropertyCode: 0x500F (ExposureIndex/ISO)
        buf.extend_from_slice(&pack_u16(0x500F));
        // DataType: UINT16 (0x0004)
        buf.extend_from_slice(&pack_u16(0x0004));
        // GetSet: read-write (0x01)
        buf.push(0x01);
        // DefaultValue: 400
        buf.extend_from_slice(&pack_u16(400));
        // CurrentValue: 800
        buf.extend_from_slice(&pack_u16(800));
        // FormFlag: Enumeration (0x02)
        buf.push(0x02);
        // NumberOfValues: 4
        buf.extend_from_slice(&pack_u16(4));
        // Values: 100, 200, 400, 800
        buf.extend_from_slice(&pack_u16(100));
        buf.extend_from_slice(&pack_u16(200));
        buf.extend_from_slice(&pack_u16(400));
        buf.extend_from_slice(&pack_u16(800));
        buf
    }

    #[test]
    fn device_prop_desc_parse_iso_enumeration() {
        let buf = build_iso_prop_desc();
        let desc = DevicePropDesc::from_bytes(&buf).unwrap();

        assert_eq!(desc.property_code, DevicePropertyCode::ExposureIndex);
        assert_eq!(desc.data_type, PropertyDataType::Uint16);
        assert!(desc.writable);
        assert_eq!(desc.default_value, PropertyValue::Uint16(400));
        assert_eq!(desc.current_value, PropertyValue::Uint16(800));
        assert_eq!(desc.form_type, PropertyFormType::Enumeration);
        assert!(desc.range.is_none());
        assert!(desc.enum_values.is_some());

        let values = desc.enum_values.unwrap();
        assert_eq!(values.len(), 4);
        assert_eq!(values[0], PropertyValue::Uint16(100));
        assert_eq!(values[1], PropertyValue::Uint16(200));
        assert_eq!(values[2], PropertyValue::Uint16(400));
        assert_eq!(values[3], PropertyValue::Uint16(800));
    }

    /// Build a DateTime property descriptor with no form.
    fn build_datetime_prop_desc() -> Vec<u8> {
        let mut buf = Vec::new();
        // PropertyCode: 0x5011 (DateTime)
        buf.extend_from_slice(&pack_u16(0x5011));
        // DataType: String (0xFFFF)
        buf.extend_from_slice(&pack_u16(0xFFFF));
        // GetSet: read-write (0x01)
        buf.push(0x01);
        // DefaultValue: empty string
        buf.push(0x00);
        // CurrentValue: "20240315T120000"
        buf.extend_from_slice(&pack_string("20240315T120000"));
        // FormFlag: None (0x00)
        buf.push(0x00);
        buf
    }

    #[test]
    fn device_prop_desc_parse_datetime_no_form() {
        let buf = build_datetime_prop_desc();
        let desc = DevicePropDesc::from_bytes(&buf).unwrap();

        assert_eq!(desc.property_code, DevicePropertyCode::DateTime);
        assert_eq!(desc.data_type, PropertyDataType::String);
        assert!(desc.writable);
        assert_eq!(desc.default_value, PropertyValue::String("".to_string()));
        assert_eq!(
            desc.current_value,
            PropertyValue::String("20240315T120000".to_string())
        );
        assert_eq!(desc.form_type, PropertyFormType::None);
        assert!(desc.range.is_none());
        assert!(desc.enum_values.is_none());
    }

    /// Build an exposure bias property descriptor with signed int16 range.
    fn build_exposure_bias_prop_desc() -> Vec<u8> {
        let mut buf = Vec::new();
        // PropertyCode: 0x5010 (ExposureBiasCompensation)
        buf.extend_from_slice(&pack_u16(0x5010));
        // DataType: INT16 (0x0003)
        buf.extend_from_slice(&pack_u16(0x0003));
        // GetSet: read-write (0x01)
        buf.push(0x01);
        // DefaultValue: 0
        buf.extend_from_slice(&pack_i16(0));
        // CurrentValue: -1000 (-1 EV)
        buf.extend_from_slice(&pack_i16(-1000));
        // FormFlag: Range (0x01)
        buf.push(0x01);
        // Range: min=-3000, max=3000, step=333
        buf.extend_from_slice(&pack_i16(-3000));
        buf.extend_from_slice(&pack_i16(3000));
        buf.extend_from_slice(&pack_i16(333));
        buf
    }

    #[test]
    fn device_prop_desc_parse_exposure_bias_signed() {
        let buf = build_exposure_bias_prop_desc();
        let desc = DevicePropDesc::from_bytes(&buf).unwrap();

        assert_eq!(
            desc.property_code,
            DevicePropertyCode::ExposureBiasCompensation
        );
        assert_eq!(desc.data_type, PropertyDataType::Int16);
        assert!(desc.writable);
        assert_eq!(desc.default_value, PropertyValue::Int16(0));
        assert_eq!(desc.current_value, PropertyValue::Int16(-1000));
        assert_eq!(desc.form_type, PropertyFormType::Range);

        let range = desc.range.unwrap();
        assert_eq!(range.min, PropertyValue::Int16(-3000));
        assert_eq!(range.max, PropertyValue::Int16(3000));
        assert_eq!(range.step, PropertyValue::Int16(333));
    }

    #[test]
    fn device_prop_desc_parse_insufficient_bytes() {
        // Too short to contain even the property code
        assert!(DevicePropDesc::from_bytes(&[0x01]).is_err());
        // Missing data type
        assert!(DevicePropDesc::from_bytes(&[0x01, 0x50]).is_err());
    }
}
