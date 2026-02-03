//! Property-related types for MTP/PTP device properties.
//!
//! This module contains types for working with device properties:
//! - [`PropertyValue`]: A property value with its associated type
//! - [`PropertyFormType`]: Form type for property value constraints
//! - [`PropertyRange`]: Range constraint for a property value
//! - [`DevicePropDesc`]: Device property descriptor

use crate::ptp::codes::{DevicePropertyCode, PropertyDataType};
use crate::ptp::pack::{
    pack_i16, pack_i32, pack_i64, pack_i8, pack_string, pack_u16, pack_u32, pack_u64, pack_u8,
    unpack_i16, unpack_i32, unpack_i64, unpack_i8, unpack_string, unpack_u16, unpack_u64, unpack_u8,
};

// --- PropertyValue Enum ---

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
                let val = crate::ptp::pack::unpack_u32(buf)?;
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

// --- PropertyFormType Enum ---

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

// --- PropertyRange Struct ---

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

// --- DevicePropDesc Structure ---

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ptp::pack::{pack_i16, pack_u16};

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
        use crate::ptp::pack::pack_string;
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

    // =========================================================================
    // Property-based tests (proptest)
    // =========================================================================

    use proptest::prelude::*;

    // -------------------------------------------------------------------------
    // PropertyFormType property tests
    // -------------------------------------------------------------------------

    proptest! {
        /// Known PropertyFormType variants roundtrip correctly
        #[test]
        fn prop_property_form_type_known_roundtrip(code in 0u8..=2u8) {
            let form = PropertyFormType::from_code(code);
            prop_assert_eq!(form.to_code(), code);
        }

        /// Unknown PropertyFormType values preserve the original code
        #[test]
        fn prop_property_form_type_unknown_preserves_code(code in 3u8..=u8::MAX) {
            let form = PropertyFormType::from_code(code);
            prop_assert_eq!(form, PropertyFormType::Unknown(code));
            prop_assert_eq!(form.to_code(), code);
        }
    }

    // -------------------------------------------------------------------------
    // PropertyValue roundtrip property tests
    // -------------------------------------------------------------------------

    proptest! {
        #[test]
        fn prop_property_value_int8_roundtrip(val: i8) {
            let pv = PropertyValue::Int8(val);
            let bytes = pv.to_bytes();
            let (parsed, consumed) = PropertyValue::from_bytes(&bytes, PropertyDataType::Int8).unwrap();
            prop_assert_eq!(parsed, pv);
            prop_assert_eq!(consumed, 1);
        }

        #[test]
        fn prop_property_value_uint8_roundtrip(val: u8) {
            let pv = PropertyValue::Uint8(val);
            let bytes = pv.to_bytes();
            let (parsed, consumed) = PropertyValue::from_bytes(&bytes, PropertyDataType::Uint8).unwrap();
            prop_assert_eq!(parsed, pv);
            prop_assert_eq!(consumed, 1);
        }

        #[test]
        fn prop_property_value_int16_roundtrip(val: i16) {
            let pv = PropertyValue::Int16(val);
            let bytes = pv.to_bytes();
            let (parsed, consumed) = PropertyValue::from_bytes(&bytes, PropertyDataType::Int16).unwrap();
            prop_assert_eq!(parsed, pv);
            prop_assert_eq!(consumed, 2);
        }

        #[test]
        fn prop_property_value_uint16_roundtrip(val: u16) {
            let pv = PropertyValue::Uint16(val);
            let bytes = pv.to_bytes();
            let (parsed, consumed) = PropertyValue::from_bytes(&bytes, PropertyDataType::Uint16).unwrap();
            prop_assert_eq!(parsed, pv);
            prop_assert_eq!(consumed, 2);
        }

        #[test]
        fn prop_property_value_int32_roundtrip(val: i32) {
            let pv = PropertyValue::Int32(val);
            let bytes = pv.to_bytes();
            let (parsed, consumed) = PropertyValue::from_bytes(&bytes, PropertyDataType::Int32).unwrap();
            prop_assert_eq!(parsed, pv);
            prop_assert_eq!(consumed, 4);
        }

        #[test]
        fn prop_property_value_uint32_roundtrip(val: u32) {
            let pv = PropertyValue::Uint32(val);
            let bytes = pv.to_bytes();
            let (parsed, consumed) = PropertyValue::from_bytes(&bytes, PropertyDataType::Uint32).unwrap();
            prop_assert_eq!(parsed, pv);
            prop_assert_eq!(consumed, 4);
        }

        #[test]
        fn prop_property_value_int64_roundtrip(val: i64) {
            let pv = PropertyValue::Int64(val);
            let bytes = pv.to_bytes();
            let (parsed, consumed) = PropertyValue::from_bytes(&bytes, PropertyDataType::Int64).unwrap();
            prop_assert_eq!(parsed, pv);
            prop_assert_eq!(consumed, 8);
        }

        #[test]
        fn prop_property_value_uint64_roundtrip(val: u64) {
            let pv = PropertyValue::Uint64(val);
            let bytes = pv.to_bytes();
            let (parsed, consumed) = PropertyValue::from_bytes(&bytes, PropertyDataType::Uint64).unwrap();
            prop_assert_eq!(parsed, pv);
            prop_assert_eq!(consumed, 8);
        }
    }

    /// Strategy for generating valid UTF-16 compatible strings for PropertyValue.
    fn valid_property_string() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop::char::range('\u{0000}', '\u{D7FF}')
                .prop_union(prop::char::range('\u{E000}', '\u{FFFF}')),
            0..50, // Keep shorter for property values
        )
        .prop_map(|chars| chars.into_iter().collect::<String>())
    }

    proptest! {
        #[test]
        fn prop_property_value_string_roundtrip(s in valid_property_string()) {
            // Limit string length for MTP compatibility
            let s = if s.chars().count() > 254 {
                s.chars().take(254).collect::<String>()
            } else {
                s
            };

            let pv = PropertyValue::String(s.clone());
            let bytes = pv.to_bytes();
            let (parsed, _consumed) = PropertyValue::from_bytes(&bytes, PropertyDataType::String).unwrap();
            prop_assert_eq!(parsed, PropertyValue::String(s));
        }
    }

    // -------------------------------------------------------------------------
    // PropertyValue data_type() consistency tests
    // -------------------------------------------------------------------------

    proptest! {
        #[test]
        fn prop_property_value_data_type_int8(val: i8) {
            let pv = PropertyValue::Int8(val);
            prop_assert_eq!(pv.data_type(), PropertyDataType::Int8);
        }

        #[test]
        fn prop_property_value_data_type_uint8(val: u8) {
            let pv = PropertyValue::Uint8(val);
            prop_assert_eq!(pv.data_type(), PropertyDataType::Uint8);
        }

        #[test]
        fn prop_property_value_data_type_int16(val: i16) {
            let pv = PropertyValue::Int16(val);
            prop_assert_eq!(pv.data_type(), PropertyDataType::Int16);
        }

        #[test]
        fn prop_property_value_data_type_uint16(val: u16) {
            let pv = PropertyValue::Uint16(val);
            prop_assert_eq!(pv.data_type(), PropertyDataType::Uint16);
        }

        #[test]
        fn prop_property_value_data_type_int32(val: i32) {
            let pv = PropertyValue::Int32(val);
            prop_assert_eq!(pv.data_type(), PropertyDataType::Int32);
        }

        #[test]
        fn prop_property_value_data_type_uint32(val: u32) {
            let pv = PropertyValue::Uint32(val);
            prop_assert_eq!(pv.data_type(), PropertyDataType::Uint32);
        }

        #[test]
        fn prop_property_value_data_type_int64(val: i64) {
            let pv = PropertyValue::Int64(val);
            prop_assert_eq!(pv.data_type(), PropertyDataType::Int64);
        }

        #[test]
        fn prop_property_value_data_type_uint64(val: u64) {
            let pv = PropertyValue::Uint64(val);
            prop_assert_eq!(pv.data_type(), PropertyDataType::Uint64);
        }
    }

    // -------------------------------------------------------------------------
    // PropertyRange roundtrip property tests
    // -------------------------------------------------------------------------

    proptest! {
        #[test]
        fn prop_property_range_uint8_roundtrip(min: u8, max: u8, step: u8) {
            let range = PropertyRange {
                min: PropertyValue::Uint8(min),
                max: PropertyValue::Uint8(max),
                step: PropertyValue::Uint8(step),
            };
            let bytes = range.to_bytes();
            let (parsed, consumed) = PropertyRange::from_bytes(&bytes, PropertyDataType::Uint8).unwrap();
            prop_assert_eq!(parsed.min, range.min);
            prop_assert_eq!(parsed.max, range.max);
            prop_assert_eq!(parsed.step, range.step);
            prop_assert_eq!(consumed, 3);
        }

        #[test]
        fn prop_property_range_uint16_roundtrip(min: u16, max: u16, step: u16) {
            let range = PropertyRange {
                min: PropertyValue::Uint16(min),
                max: PropertyValue::Uint16(max),
                step: PropertyValue::Uint16(step),
            };
            let bytes = range.to_bytes();
            let (parsed, consumed) = PropertyRange::from_bytes(&bytes, PropertyDataType::Uint16).unwrap();
            prop_assert_eq!(parsed.min, range.min);
            prop_assert_eq!(parsed.max, range.max);
            prop_assert_eq!(parsed.step, range.step);
            prop_assert_eq!(consumed, 6);
        }

        #[test]
        fn prop_property_range_int16_roundtrip(min: i16, max: i16, step: i16) {
            let range = PropertyRange {
                min: PropertyValue::Int16(min),
                max: PropertyValue::Int16(max),
                step: PropertyValue::Int16(step),
            };
            let bytes = range.to_bytes();
            let (parsed, consumed) = PropertyRange::from_bytes(&bytes, PropertyDataType::Int16).unwrap();
            prop_assert_eq!(parsed.min, range.min);
            prop_assert_eq!(parsed.max, range.max);
            prop_assert_eq!(parsed.step, range.step);
            prop_assert_eq!(consumed, 6);
        }

        #[test]
        fn prop_property_range_uint32_roundtrip(min: u32, max: u32, step: u32) {
            let range = PropertyRange {
                min: PropertyValue::Uint32(min),
                max: PropertyValue::Uint32(max),
                step: PropertyValue::Uint32(step),
            };
            let bytes = range.to_bytes();
            let (parsed, consumed) = PropertyRange::from_bytes(&bytes, PropertyDataType::Uint32).unwrap();
            prop_assert_eq!(parsed.min, range.min);
            prop_assert_eq!(parsed.max, range.max);
            prop_assert_eq!(parsed.step, range.step);
            prop_assert_eq!(consumed, 12);
        }
    }

    // =========================================================================
    // ADVERSARIAL PROPERTY-BASED TESTS
    // Goal: Find bugs by testing malformed/invalid/truncated inputs
    // =========================================================================

    // -------------------------------------------------------------------------
    // PropertyValue from_bytes with mismatched/truncated data
    // -------------------------------------------------------------------------

    proptest! {
        /// PropertyValue from_bytes with potentially wrong-sized buffer
        #[test]
        fn fuzz_property_value_truncated(
            data_type_code in 1u16..=8u16, // Int8 through Uint64
            bytes in prop::collection::vec(any::<u8>(), 0..20)
        ) {
            let data_type = PropertyDataType::from_code(data_type_code);
            // Try to parse with potentially wrong-sized buffer - should not panic
            let _ = PropertyValue::from_bytes(&bytes, data_type);
        }

        /// PropertyValue from_bytes with empty buffer
        #[test]
        fn fuzz_property_value_empty(data_type_code in 1u16..=8u16) {
            let data_type = PropertyDataType::from_code(data_type_code);
            let result = PropertyValue::from_bytes(&[], data_type);
            // Empty buffer should fail for all types
            prop_assert!(result.is_err());
        }

        /// PropertyValue from_bytes with String type and garbage
        #[test]
        fn fuzz_property_value_string_garbage(bytes in prop::collection::vec(any::<u8>(), 0..50)) {
            let result = PropertyValue::from_bytes(&bytes, PropertyDataType::String);
            // Should not panic
            let _ = result;
        }

        /// PropertyValue from_bytes with unsupported types
        #[test]
        fn fuzz_property_value_unsupported(bytes in prop::collection::vec(any::<u8>(), 0..20)) {
            // Undefined, Int128, Uint128 are not supported
            let result_undefined = PropertyValue::from_bytes(&bytes, PropertyDataType::Undefined);
            prop_assert!(result_undefined.is_err());

            let result_int128 = PropertyValue::from_bytes(&bytes, PropertyDataType::Int128);
            prop_assert!(result_int128.is_err());

            let result_uint128 = PropertyValue::from_bytes(&bytes, PropertyDataType::Uint128);
            prop_assert!(result_uint128.is_err());
        }

        /// PropertyValue from_bytes with Unknown data type
        #[test]
        fn fuzz_property_value_unknown_type(
            unknown_code in 11u16..=0xFFFEu16, // Not in 0-10 range and not String (0xFFFF)
            bytes in prop::collection::vec(any::<u8>(), 0..20)
        ) {
            let data_type = PropertyDataType::Unknown(unknown_code);
            let result = PropertyValue::from_bytes(&bytes, data_type);
            prop_assert!(result.is_err());
        }
    }

    // -------------------------------------------------------------------------
    // DevicePropDesc with truncated/corrupted data
    // -------------------------------------------------------------------------

    proptest! {
        /// DevicePropDesc with truncated data should fail gracefully
        #[test]
        fn fuzz_device_prop_desc_truncated(bytes in prop::collection::vec(any::<u8>(), 0..50)) {
            let _ = DevicePropDesc::from_bytes(&bytes);
        }

        /// DevicePropDesc with random garbage should not panic
        #[test]
        fn fuzz_device_prop_desc_garbage(bytes in prop::collection::vec(any::<u8>(), 0..200)) {
            let _ = DevicePropDesc::from_bytes(&bytes);
        }
    }

    // -------------------------------------------------------------------------
    // PropertyRange with truncated data
    // -------------------------------------------------------------------------

    proptest! {
        /// PropertyRange with truncated data should fail gracefully
        #[test]
        fn fuzz_property_range_truncated(
            data_type_code in 1u16..=8u16,
            bytes in prop::collection::vec(any::<u8>(), 0..20)
        ) {
            let data_type = PropertyDataType::from_code(data_type_code);
            let _ = PropertyRange::from_bytes(&bytes, data_type);
        }

        /// PropertyRange with wrong data type
        #[test]
        fn fuzz_property_range_wrong_type(bytes in prop::collection::vec(any::<u8>(), 0..20)) {
            // Undefined type
            let result = PropertyRange::from_bytes(&bytes, PropertyDataType::Undefined);
            prop_assert!(result.is_err());

            // Unknown type
            let result = PropertyRange::from_bytes(&bytes, PropertyDataType::Unknown(0x1234));
            prop_assert!(result.is_err());
        }
    }

    // -------------------------------------------------------------------------
    // Boundary tests
    // -------------------------------------------------------------------------

    #[test]
    fn device_prop_desc_minimum_valid() {
        // DevicePropDesc needs: PropertyCode(2) + DataType(2) + GetSet(1) + Default + Current + FormFlag(1)
        // Minimum with Uint8 values: 2 + 2 + 1 + 1 + 1 + 1 = 8 bytes
        assert!(DevicePropDesc::from_bytes(&[]).is_err());
        assert!(DevicePropDesc::from_bytes(&[0; 4]).is_err());
    }

    // -------------------------------------------------------------------------
    // Test PropertyValue size boundaries
    // -------------------------------------------------------------------------

    #[test]
    fn property_value_int8_boundary() {
        let bytes = [0x80]; // -128 in signed
        let (val, consumed) = PropertyValue::from_bytes(&bytes, PropertyDataType::Int8).unwrap();
        assert!(matches!(val, PropertyValue::Int8(-128)));
        assert_eq!(consumed, 1);

        let bytes = [0x7F]; // 127 in signed
        let (val, _) = PropertyValue::from_bytes(&bytes, PropertyDataType::Int8).unwrap();
        assert!(matches!(val, PropertyValue::Int8(127)));
    }

    #[test]
    fn property_value_int16_boundary() {
        let bytes = [0x00, 0x80]; // -32768 in little-endian
        let (val, consumed) = PropertyValue::from_bytes(&bytes, PropertyDataType::Int16).unwrap();
        assert!(matches!(val, PropertyValue::Int16(-32768)));
        assert_eq!(consumed, 2);
    }

    #[test]
    fn property_value_int32_boundary() {
        let bytes = [0x00, 0x00, 0x00, 0x80]; // i32::MIN in little-endian
        let (val, consumed) = PropertyValue::from_bytes(&bytes, PropertyDataType::Int32).unwrap();
        assert!(matches!(val, PropertyValue::Int32(i32::MIN)));
        assert_eq!(consumed, 4);
    }

    #[test]
    fn property_value_int64_boundary() {
        let bytes = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80]; // i64::MIN in little-endian
        let (val, consumed) = PropertyValue::from_bytes(&bytes, PropertyDataType::Int64).unwrap();
        assert!(matches!(val, PropertyValue::Int64(i64::MIN)));
        assert_eq!(consumed, 8);
    }
}
