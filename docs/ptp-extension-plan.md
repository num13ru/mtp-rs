# PTP device property and capture extension plan

## Overview

This document describes the plan to extend the PTP implementation in `mtp-rs` to support:

1. **Device property operations** - Read and modify camera/device settings
2. **Capture operations** - Trigger photo capture on PTP cameras

These features primarily target digital cameras (the original PTP use case) rather than Android devices, enabling remote
camera control applications.

### Why this matters

- **Camera control**: Adjust ISO, aperture, shutter speed, etc. remotely
- **Tethered shooting**: Trigger captures and retrieve images
- **Camera apps**: Build photo booth applications, time-lapse controllers, etc.

## Architecture

### Where new code lives

All additions go in the `ptp::` module. The high-level `mtp::` module remains unchanged since device properties and
capture are camera-specific features not typically used with Android MTP devices.

```
src/ptp/
├── codes.rs          # Add DevicePropertyCode enum, new OperationCode variants
├── pack.rs           # Add signed integer pack/unpack (i8, i16, i32, i64)
├── types.rs          # Add DevicePropDesc, PropertyValue, FormType structs
├── session.rs        # Add property and capture methods to PtpSession
└── mod.rs            # Export new public types
```

### Module dependencies

The new code follows the existing dependency rules:

```
ptp::session  ──depends on──► ptp::types, ptp::codes, ptp::pack
ptp::types    ──depends on──► ptp::pack, ptp::codes
ptp::codes    ──depends on──► (nothing internal)
ptp::pack     ──depends on──► error
```

## New types needed

### DevicePropertyCode enum

Similar to existing `ObjectPropertyCode` and `ObjectFormatCode` patterns.

```rust
/// Standard PTP device property codes (0x5000 range).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum DevicePropertyCode {
    /// Battery level (UINT8, 0-100).
    BatteryLevel = 0x5001,
    /// Device functional mode (UINT16).
    FunctionalMode = 0x5002,
    /// Image size setting (String, e.g., "1920x1080").
    ImageSize = 0x5003,
    /// Compression setting (UINT8).
    CompressionSetting = 0x5004,
    /// White balance (UINT16).
    WhiteBalance = 0x5005,
    /// RGB gain (String).
    RgbGain = 0x5006,
    /// F-Number/Aperture (UINT16, value/100 = f-stop).
    FNumber = 0x5007,
    /// Focal length (UINT32, units of 0.01mm).
    FocalLength = 0x5008,
    /// Focus distance (UINT16, mm).
    FocusDistance = 0x5009,
    /// Focus mode (UINT16).
    FocusMode = 0x500A,
    /// Exposure metering mode (UINT16).
    ExposureMeteringMode = 0x500B,
    /// Flash mode (UINT16).
    FlashMode = 0x500C,
    /// Exposure time/shutter speed (UINT32, units of 0.0001s).
    ExposureTime = 0x500D,
    /// Exposure program mode (UINT16).
    ExposureProgramMode = 0x500E,
    /// Exposure index/ISO (UINT16).
    ExposureIndex = 0x500F,
    /// Exposure bias compensation (INT16, units of 0.001 EV).
    ExposureBiasCompensation = 0x5010,
    /// Date and time (String, "YYYYMMDDThhmmss").
    DateTime = 0x5011,
    /// Capture delay (UINT32, ms).
    CaptureDelay = 0x5012,
    /// Still capture mode (UINT16).
    StillCaptureMode = 0x5013,
    /// Unknown/vendor-specific property code.
    Unknown(u16),
}
```

### PropertyDataType enum

Data type codes used in property descriptors.

```rust
/// PTP property data type codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum PropertyDataType {
    /// Undefined type.
    Undefined = 0x0000,
    /// Signed 8-bit integer.
    Int8 = 0x0001,
    /// Unsigned 8-bit integer.
    Uint8 = 0x0002,
    /// Signed 16-bit integer.
    Int16 = 0x0003,
    /// Unsigned 16-bit integer.
    Uint16 = 0x0004,
    /// Signed 32-bit integer.
    Int32 = 0x0005,
    /// Unsigned 32-bit integer.
    Uint32 = 0x0006,
    /// Signed 64-bit integer.
    Int64 = 0x0007,
    /// Unsigned 64-bit integer.
    Uint64 = 0x0008,
    /// UTF-16LE string.
    String = 0xFFFF,
    /// Unknown type code.
    Unknown(u16),
}
```

### PropertyFormType enum

How allowed values are specified.

```rust
/// Form type for property value constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PropertyFormType {
    /// No constraints (any value valid).
    None = 0x00,
    /// Value must be within a range (min, max, step).
    Range = 0x01,
    /// Value must be one of an enumerated set.
    Enumeration = 0x02,
}
```

### PropertyValue enum

Represents a property value of any supported type.

```rust
/// A property value with its associated type.
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
```

### PropertyRange struct

Constraints for range-form properties.

```rust
/// Range constraint for a property value.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyRange {
    /// Minimum allowed value.
    pub min: PropertyValue,
    /// Maximum allowed value.
    pub max: PropertyValue,
    /// Step size between allowed values.
    pub step: PropertyValue,
}
```

### DevicePropDesc struct

The complete property descriptor as returned by GetDevicePropDesc.

```rust
/// Device property descriptor.
///
/// Describes a device property including its type, current value,
/// default value, and allowed values/ranges.
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
```

## New operations

### OperationCode additions

Add new variants to the existing `OperationCode` enum:

```rust
/// Get device property descriptor.
GetDevicePropDesc = 0x1014,
/// Get current device property value.
GetDevicePropValue = 0x1015,
/// Set device property value.
SetDevicePropValue = 0x1016,
/// Reset device property to default.
ResetDevicePropValue = 0x1017,
/// Initiate image capture.
InitiateCapture = 0x100E,
```

### EventCode additions

Add new event for capture completion:

```rust
/// Capture operation completed.
CaptureComplete = 0x400D,
```

### ResponseCode additions

Add property-specific error codes:

```rust
/// Device property not supported.
DevicePropNotSupported = 0x200A,
/// Invalid device property value.
InvalidDevicePropValue = 0x200B,
/// Invalid device property format.
InvalidDevicePropFormat = 0x200C,
```

### PtpSession method additions

```rust
impl PtpSession {
    /// Get the descriptor for a device property.
    ///
    /// Returns detailed information about the property including its type,
    /// current value, default value, and allowed values/range.
    pub async fn get_device_prop_desc(
        &self,
        property: DevicePropertyCode,
    ) -> Result<DevicePropDesc, Error>;

    /// Get the current value of a device property.
    ///
    /// For simple value retrieval without full descriptor information.
    pub async fn get_device_prop_value(
        &self,
        property: DevicePropertyCode,
    ) -> Result<PropertyValue, Error>;

    /// Set a device property value.
    ///
    /// The value type must match the property's data type.
    pub async fn set_device_prop_value(
        &self,
        property: DevicePropertyCode,
        value: &PropertyValue,
    ) -> Result<(), Error>;

    /// Reset a device property to its default value.
    pub async fn reset_device_prop_value(
        &self,
        property: DevicePropertyCode,
    ) -> Result<(), Error>;

    /// Initiate a capture operation.
    ///
    /// Triggers the camera to capture an image. The capture is asynchronous;
    /// use poll_event() to wait for CaptureComplete and ObjectAdded events.
    ///
    /// # Arguments
    ///
    /// * `storage_id` - Target storage (use StorageId(0) for camera default)
    /// * `format` - Object format for the capture (use ObjectFormatCode::Undefined
    ///   for camera default)
    pub async fn initiate_capture(
        &self,
        storage_id: StorageId,
        format: ObjectFormatCode,
    ) -> Result<(), Error>;
}
```

## Data serialization

### Property descriptor format

The DevicePropDesc dataset is serialized as follows (all little-endian):

| Offset | Size | Field        | Description                                |
|--------|------|--------------|--------------------------------------------|
| 0      | 2    | PropertyCode | Device property code (u16)                 |
| 2      | 2    | DataType     | Data type code (u16)                       |
| 4      | 1    | GetSet       | 0x00 = read-only, 0x01 = read-write        |
| 5      | var  | DefaultValue | Default value (size depends on DataType)   |
| var    | var  | CurrentValue | Current value (size depends on DataType)   |
| var    | 1    | FormFlag     | 0x00 = None, 0x01 = Range, 0x02 = Enum     |
| var    | var  | Form         | Range or Enum form data (if FormFlag != 0) |

### Value serialization by type

| Type         | Size     | Format                                          |
|--------------|----------|-------------------------------------------------|
| Int8/Uint8   | 1        | Direct byte                                     |
| Int16/Uint16 | 2        | Little-endian                                   |
| Int32/Uint32 | 4        | Little-endian                                   |
| Int64/Uint64 | 8        | Little-endian                                   |
| String       | Variable | Length prefix (u8) + UTF-16LE + null terminator |

### Range form format

| Offset | Size | Field        |
|--------|------|--------------|
| 0      | var  | MinimumValue |
| var    | var  | MaximumValue |
| var    | var  | StepSize     |

### Enumeration form format

| Offset | Size | Field                |
|--------|------|----------------------|
| 0      | 2    | NumberOfValues (u16) |
| 2      | var  | SupportedValue[0]    |
| var    | var  | SupportedValue[1]    |
| ...    | ...  | ...                  |

### New pack/unpack functions needed

Add signed integer support to `pack.rs`:

```rust
// Signed packing
pub fn pack_i8(val: i8) -> [u8; 1];
pub fn pack_i16(val: i16) -> [u8; 2];
pub fn pack_i32(val: i32) -> [u8; 4];
pub fn pack_i64(val: i64) -> [u8; 8];

// Signed unpacking
pub fn unpack_i8(buf: &[u8]) -> Result<i8, Error>;
pub fn unpack_i16(buf: &[u8]) -> Result<i16, Error>;
pub fn unpack_i32(buf: &[u8]) -> Result<i32, Error>;
pub fn unpack_i64(buf: &[u8]) -> Result<i64, Error>;
```

## Error handling

### New error cases

The existing `Error::Protocol` variant handles device errors. Add recognition of:

- `ResponseCode::DevicePropNotSupported` (0x200A) - Property not available
- `ResponseCode::InvalidDevicePropValue` (0x200B) - Value rejected
- `ResponseCode::InvalidDevicePropFormat` (0x200C) - Wrong type

### Validation errors

Add validation in `set_device_prop_value()`:

1. Check that value type matches the property's declared data type
2. If property has range form, validate value is within range
3. If property has enum form, validate value is in allowed set

These should return `Error::InvalidData` with descriptive messages.

## Testing strategy

### Unit tests with MockTransport

Test serialization/deserialization of property descriptors:

```rust
#[tokio::test]
async fn test_get_device_prop_desc_battery_level() {
    let (transport, mock) = mock_transport();
    mock.queue_response(ok_response(1)); // OpenSession

    // Queue DevicePropDesc data for BatteryLevel
    let prop_desc_data = build_battery_level_prop_desc();
    mock.queue_response(data_container(2, OperationCode::GetDevicePropDesc, &prop_desc_data));
    mock.queue_response(ok_response(2));

    let session = PtpSession::open(transport, 1).await.unwrap();
    let desc = session.get_device_prop_desc(DevicePropertyCode::BatteryLevel).await.unwrap();

    assert_eq!(desc.property_code, DevicePropertyCode::BatteryLevel);
    assert_eq!(desc.data_type, PropertyDataType::Uint8);
    assert!(!desc.writable);
    assert_eq!(desc.form_type, PropertyFormType::Range);
}
```

### Property-based testing

Use proptest for serialization roundtrips:

```rust
proptest! {
    #[test]
    fn property_value_roundtrip(value: u32) {
        let pv = PropertyValue::Uint32(value);
        let bytes = pv.to_bytes(PropertyDataType::Uint32);
        let (parsed, _) = PropertyValue::from_bytes(&bytes, PropertyDataType::Uint32).unwrap();
        assert_eq!(pv, parsed);
    }
}
```

### Integration tests

Add to `tests/integration.rs` under a new `camera` module:

```rust
/// Test reading device properties (requires camera, not just Android).
#[tokio::test]
#[ignore]
#[serial]
async fn test_get_battery_level() {
    let transport = NusbTransport::open_first().await.unwrap();
    let device = PtpDevice::new(transport);
    let session = device.open_session().await.unwrap();

    // Check if device supports this property
    let info = session.get_device_info().await.unwrap();
    if !info.device_properties_supported.contains(&0x5001) {
        println!("Device doesn't support BatteryLevel property, skipping");
        return;
    }

    let desc = session.get_device_prop_desc(DevicePropertyCode::BatteryLevel).await.unwrap();
    println!("Battery level: {:?}", desc.current_value);
}
```

## Example usage

### Reading camera settings

```rust
use mtp_rs::ptp::{PtpDevice, DevicePropertyCode};

let session = device.open_session().await?;

// Get all supported properties
let info = session.get_device_info().await?;
for prop_code in & info.device_properties_supported {
let prop = DevicePropertyCode::from_code( * prop_code);
match session.get_device_prop_desc(prop).await {
Ok(desc) => {
println ! ("{:?}: {:?}", prop, desc.current_value);
if desc.writable {
println ! ("  Allowed: {:?}", desc.enum_values.or(desc.range));
}
}
Err(e) => println ! ("{:?}: Error - {}", prop, e),
}
}
```

### Setting camera exposure

```rust
use mtp_rs::ptp::{DevicePropertyCode, PropertyValue};

// Set ISO to 400
let iso = PropertyValue::Uint16(400);
session.set_device_prop_value(DevicePropertyCode::ExposureIndex, & iso).await?;

// Set aperture to f/2.8 (value is f-stop * 100)
let aperture = PropertyValue::Uint16(280);
session.set_device_prop_value(DevicePropertyCode::FNumber, & aperture).await?;
```

### Triggering capture

```rust
use mtp_rs::ptp::{StorageId, ObjectFormatCode, EventCode};

// Trigger capture
session.initiate_capture(StorageId(0), ObjectFormatCode::Undefined).await?;

// Wait for capture events
loop {
match session.poll_event().await ? {
Some(event) if event.code == EventCode::CaptureComplete => {
println ! ("Capture complete!");
break;
}
Some(event) if event.code == EventCode::ObjectAdded => {
let handle = ObjectHandle(event.params[0]);
println ! ("New object created: {:?}", handle);
// Download the captured image
let data = session.get_object(handle).await ?;
}
_ => continue,
}
}
```

## Compatibility notes

### Android devices

Most Android devices in MTP mode do not support device properties or capture operations. These operations are primarily
for digital cameras. The library should:

1. Check `DeviceInfo.device_properties_supported` before accessing properties
2. Return `Error::Protocol { code: OperationNotSupported, .. }` gracefully
3. Document that these features are for cameras, not phones

### Vendor extensions

Many cameras use vendor-specific property codes (0xD000-0xDFFF range). The `DevicePropertyCode::Unknown(u16)` variant
handles these. Future work could add vendor-specific extensions as feature flags.
