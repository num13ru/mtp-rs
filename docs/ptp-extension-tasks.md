# PTP extension implementation tasks

This document breaks down the implementation into concrete tasks, ordered by dependency.

## Implementation status

All tasks have been completed. See `docs/ptp-camera-features.md` for usage documentation.

| Phase | Task                                            | Status |
|-------|-------------------------------------------------|--------|
| 1     | 1.1 Add signed integer pack/unpack functions    | Done   |
| 1     | 1.2 Add PropertyDataType enum                   | Done   |
| 1     | 1.3 Add DevicePropertyCode enum                 | Done   |
| 1     | 1.4 Add new OperationCode variants              | Done   |
| 1     | 1.5 Add new EventCode and ResponseCode variants | Done   |
| 1     | 1.6 Add PropertyValue enum                      | Done   |
| 1     | 1.7 Add PropertyFormType enum                   | Done   |
| 1     | 1.8 Add PropertyRange struct                    | Done   |
| 1     | 1.9 Add DevicePropDesc struct                   | Done   |
| 2     | 2.1 Add get_device_prop_desc method             | Done   |
| 2     | 2.2 Add get_device_prop_value method            | Done   |
| 2     | 2.3 Add set_device_prop_value method            | Done   |
| 2     | 2.4 Add reset_device_prop_value method          | Done   |
| 2     | 2.5 Add typed property value helpers            | Done   |
| 3     | 3.1 Add initiate_capture method                 | Done   |
| 4     | 4.1 Add property parsing test fixtures          | Done   |
| 4     | 4.2 Add comprehensive unit tests                | Done   |
| 4     | 4.3 Add integration tests                       | Done   |
| 4     | 4.4 Update protocol documentation               | Done   |

Note: Task 3.2 (capture_and_wait helper) was not implemented as it requires runtime-specific code. Users can implement
this themselves using `initiate_capture()` and `poll_event()`.

---

## Phase 1: Types and codes

Foundation types that other code depends on.

---

### Task 1.1: Add signed integer pack/unpack functions

**Description**: Add support for serializing and deserializing signed integers. Property values can be signed (e.g.,
exposure bias compensation is INT16).

**Files to modify**:

- `src/ptp/pack.rs` - Add pack/unpack functions
- `src/ptp/mod.rs` - Export new functions

**Implementation details**:

```rust
// In pack.rs

/// Pack a signed 8-bit integer.
#[inline]
pub fn pack_i8(val: i8) -> [u8; 1] {
    [val as u8]
}

/// Unpack a signed 8-bit integer.
pub fn unpack_i8(buf: &[u8]) -> Result<i8, crate::Error> {
    if buf.is_empty() {
        return Err(crate::Error::invalid_data("insufficient bytes for i8"));
    }
    Ok(buf[0] as i8)
}

// Similarly for i16, i32, i64 using to_le_bytes() / from_le_bytes()
```

**Tests to add**:

- `pack_i8_test`, `pack_i16_little_endian`, etc.
- `unpack_i8_test`, `unpack_i16_little_endian`, etc.
- `roundtrip_i8`, `roundtrip_i16`, etc.
- Negative value tests

**Estimated complexity**: Small

**Dependencies**: None

---

### Task 1.2: Add PropertyDataType enum

**Description**: Add enum for PTP data type codes used in property descriptors.

**Files to modify**:

- `src/ptp/codes.rs` - Add PropertyDataType enum

**Implementation details**:

```rust
/// PTP property data type codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum PropertyDataType {
    Undefined = 0x0000,
    Int8 = 0x0001,
    Uint8 = 0x0002,
    Int16 = 0x0003,
    Uint16 = 0x0004,
    Int32 = 0x0005,
    Uint32 = 0x0006,
    Int64 = 0x0007,
    Uint64 = 0x0008,
    String = 0xFFFF,
    Unknown(u16),
}

impl PropertyDataType {
    pub fn from_code(code: u16) -> Self { ... }
    pub fn to_code(self) -> u16 { ... }

    /// Returns the byte size of this data type (None for variable-length String).
    pub fn byte_size(&self) -> Option<usize> {
        match self {
            Self::Int8 | Self::Uint8 => Some(1),
            Self::Int16 | Self::Uint16 => Some(2),
            Self::Int32 | Self::Uint32 => Some(4),
            Self::Int64 | Self::Uint64 => Some(8),
            Self::String | Self::Undefined | Self::Unknown(_) => None,
        }
    }
}
```

**Tests to add**:

- `property_data_type_from_code`
- `property_data_type_to_code`
- `property_data_type_roundtrip`
- `property_data_type_byte_size`

**Estimated complexity**: Small

**Dependencies**: None

---

### Task 1.3: Add DevicePropertyCode enum

**Description**: Add enum for standard device property codes (0x5000 range).

**Files to modify**:

- `src/ptp/codes.rs` - Add DevicePropertyCode enum
- `src/ptp/mod.rs` - Export DevicePropertyCode

**Implementation details**: Follow the pattern of existing `ObjectPropertyCode`. Include all properties listed in the
plan document.

**Tests to add**:

- `device_property_code_from_known_codes`
- `device_property_code_to_known_codes`
- `device_property_code_unknown_roundtrip`
- `device_property_code_known_roundtrip`

**Estimated complexity**: Small

**Dependencies**: None

---

### Task 1.4: Add new OperationCode variants

**Description**: Add device property and capture operation codes to the existing enum.

**Files to modify**:

- `src/ptp/codes.rs` - Add variants to OperationCode enum

**Variants to add**:

- `GetDevicePropDesc = 0x1014`
- `GetDevicePropValue = 0x1015`
- `SetDevicePropValue = 0x1016`
- `ResetDevicePropValue = 0x1017`
- `InitiateCapture = 0x100E`

**Implementation details**: Update both `from_code()` and `to_code()` match arms.

**Tests to add**: Update existing roundtrip tests to include new variants.

**Estimated complexity**: Small

**Dependencies**: None

---

### Task 1.5: Add new EventCode and ResponseCode variants

**Description**: Add capture-related event code and property-related response codes.

**Files to modify**:

- `src/ptp/codes.rs` - Add variants to EventCode and ResponseCode

**EventCode to add**:

- `CaptureComplete = 0x400D`

**ResponseCode to add**:

- `DevicePropNotSupported = 0x200A`
- `InvalidDevicePropValue = 0x200B`
- `InvalidDevicePropFormat = 0x200C` (Note: verify this doesn't conflict with existing codes)

**Estimated complexity**: Small

**Dependencies**: None

---

### Task 1.6: Add PropertyValue enum

**Description**: Add enum to represent property values of different types.

**Files to modify**:

- `src/ptp/types.rs` - Add PropertyValue enum

**Implementation details**:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    Int8(i8),
    Uint8(u8),
    Int16(i16),
    Uint16(u16),
    Int32(i32),
    Uint32(u32),
    Int64(i64),
    Uint64(u64),
    String(String),
}

impl PropertyValue {
    /// Serialize value to bytes according to PTP format.
    pub fn to_bytes(&self) -> Vec<u8> { ... }

    /// Parse value from bytes given the expected data type.
    pub fn from_bytes(buf: &[u8], data_type: PropertyDataType)
                      -> Result<(Self, usize), crate::Error> { ... }
}
```

**Tests to add**:

- `property_value_to_bytes_*` for each variant
- `property_value_from_bytes_*` for each type
- `property_value_roundtrip_*` for each type
- Error cases for insufficient bytes

**Estimated complexity**: Medium

**Dependencies**: Task 1.1 (signed pack/unpack), Task 1.2 (PropertyDataType)

---

### Task 1.7: Add PropertyFormType enum

**Description**: Add enum for property form types (None, Range, Enumeration).

**Files to modify**:

- `src/ptp/types.rs` - Add PropertyFormType enum

**Implementation details**:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PropertyFormType {
    None = 0x00,
    Range = 0x01,
    Enumeration = 0x02,
}

impl PropertyFormType {
    pub fn from_code(code: u8) -> Self { ... }
    pub fn to_code(self) -> u8 { ... }
}
```

**Estimated complexity**: Small

**Dependencies**: None

---

### Task 1.8: Add PropertyRange struct

**Description**: Add struct for range-form property constraints.

**Files to modify**:

- `src/ptp/types.rs` - Add PropertyRange struct

**Implementation details**:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyRange {
    pub min: PropertyValue,
    pub max: PropertyValue,
    pub step: PropertyValue,
}

impl PropertyRange {
    pub fn from_bytes(buf: &[u8], data_type: PropertyDataType)
                      -> Result<(Self, usize), crate::Error> { ... }

    pub fn to_bytes(&self) -> Vec<u8> { ... }
}
```

**Tests to add**:

- Parse range for various data types
- Roundtrip tests

**Estimated complexity**: Small

**Dependencies**: Task 1.6 (PropertyValue)

---

### Task 1.9: Add DevicePropDesc struct

**Description**: Add the main device property descriptor struct.

**Files to modify**:

- `src/ptp/types.rs` - Add DevicePropDesc struct
- `src/ptp/mod.rs` - Export DevicePropDesc

**Implementation details**:

```rust
#[derive(Debug, Clone)]
pub struct DevicePropDesc {
    pub property_code: DevicePropertyCode,
    pub data_type: PropertyDataType,
    pub writable: bool,
    pub default_value: PropertyValue,
    pub current_value: PropertyValue,
    pub form_type: PropertyFormType,
    pub enum_values: Option<Vec<PropertyValue>>,
    pub range: Option<PropertyRange>,
}

impl DevicePropDesc {
    /// Parse a DevicePropDesc from bytes.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::Error> { ... }
}
```

**Tests to add**:

- Parse minimal descriptor (form_type = None)
- Parse descriptor with range form
- Parse descriptor with enumeration form
- Parse descriptors of various data types
- Error cases for malformed data

**Estimated complexity**: Large

**Dependencies**: Task 1.3, Task 1.6, Task 1.7, Task 1.8

---

## Phase 2: Property operations

Add session methods for property operations.

---

### Task 2.1: Add get_device_prop_desc method

**Description**: Add method to retrieve a property descriptor from the device.

**Files to modify**:

- `src/ptp/session.rs` - Add get_device_prop_desc method

**Implementation details**:

```rust
pub async fn get_device_prop_desc(
    &self,
    property: DevicePropertyCode,
) -> Result<DevicePropDesc, Error> {
    let (response, data) = self
        .execute_with_receive(
            OperationCode::GetDevicePropDesc,
            &[property.to_code() as u32],
        )
        .await?;
    Self::check_response(response, OperationCode::GetDevicePropDesc)?;
    DevicePropDesc::from_bytes(&data)
}
```

**Tests to add**:

- Mock test with queued prop desc response
- Test various property types
- Test error handling for unsupported property

**Estimated complexity**: Medium

**Dependencies**: Task 1.4, Task 1.9

---

### Task 2.2: Add get_device_prop_value method

**Description**: Add method to get just the current value of a property (without full descriptor).

**Files to modify**:

- `src/ptp/session.rs` - Add get_device_prop_value method

**Implementation details**:

```rust
pub async fn get_device_prop_value(
    &self,
    property: DevicePropertyCode,
) -> Result<Vec<u8>, Error> {
    let (response, data) = self
        .execute_with_receive(
            OperationCode::GetDevicePropValue,
            &[property.to_code() as u32],
        )
        .await?;
    Self::check_response(response, OperationCode::GetDevicePropValue)?;
    Ok(data)
}
```

Note: Returns raw bytes because parsing requires knowing the data type. Caller can use `DevicePropDesc` first to get
type info.

**Tests to add**:

- Mock test with queued value response
- Test for various value types

**Estimated complexity**: Small

**Dependencies**: Task 1.4

---

### Task 2.3: Add set_device_prop_value method

**Description**: Add method to set a property value.

**Files to modify**:

- `src/ptp/session.rs` - Add set_device_prop_value method

**Implementation details**:

```rust
pub async fn set_device_prop_value(
    &self,
    property: DevicePropertyCode,
    value: &[u8],
) -> Result<(), Error> {
    let response = self
        .execute_with_send(
            OperationCode::SetDevicePropValue,
            &[property.to_code() as u32],
            value,
        )
        .await?;
    Self::check_response(response, OperationCode::SetDevicePropValue)?;
    Ok(())
}
```

**Tests to add**:

- Mock test setting a property
- Test error handling for read-only property
- Test error handling for invalid value

**Estimated complexity**: Small

**Dependencies**: Task 1.4

---

### Task 2.4: Add reset_device_prop_value method

**Description**: Add method to reset a property to its default value.

**Files to modify**:

- `src/ptp/session.rs` - Add reset_device_prop_value method

**Implementation details**:

```rust
pub async fn reset_device_prop_value(
    &self,
    property: DevicePropertyCode,
) -> Result<(), Error> {
    let response = self
        .execute(
            OperationCode::ResetDevicePropValue,
            &[property.to_code() as u32],
        )
        .await?;
    Self::check_response(response, OperationCode::ResetDevicePropValue)?;
    Ok(())
}
```

**Tests to add**:

- Mock test resetting a property
- Test error handling

**Estimated complexity**: Small

**Dependencies**: Task 1.4

---

### Task 2.5: Add typed property value helpers

**Description**: Add convenience methods that handle value parsing/serialization.

**Files to modify**:

- `src/ptp/session.rs` - Add helper methods

**Implementation details**:

```rust
/// Get a property value as a specific type.
pub async fn get_device_prop_value_typed(
    &self,
    property: DevicePropertyCode,
    data_type: PropertyDataType,
) -> Result<PropertyValue, Error> {
    let data = self.get_device_prop_value(property).await?;
    let (value, _) = PropertyValue::from_bytes(&data, data_type)?;
    Ok(value)
}

/// Set a property value from a PropertyValue.
pub async fn set_device_prop_value_typed(
    &self,
    property: DevicePropertyCode,
    value: &PropertyValue,
) -> Result<(), Error> {
    let data = value.to_bytes();
    self.set_device_prop_value(property, &data).await
}
```

**Tests to add**:

- Mock test for typed get/set
- Test type mismatch handling

**Estimated complexity**: Small

**Dependencies**: Task 2.2, Task 2.3, Task 1.6

---

## Phase 3: Capture operations

Add capture functionality.

---

### Task 3.1: Add initiate_capture method

**Description**: Add method to trigger a capture on the camera.

**Files to modify**:

- `src/ptp/session.rs` - Add initiate_capture method

**Implementation details**:

```rust
/// Initiate a capture operation.
///
/// This triggers the camera to capture an image. The operation is asynchronous;
/// use poll_event() to wait for CaptureComplete and ObjectAdded events.
///
/// # Arguments
///
/// * `storage_id` - Target storage (use StorageId(0) for camera default)
/// * `format` - Object format for the capture (use ObjectFormatCode::Undefined for default)
///
/// # Events
///
/// After calling this method, monitor for these events:
/// - `EventCode::CaptureComplete` - Capture operation finished
/// - `EventCode::ObjectAdded` - New object (image) was created on device
pub async fn initiate_capture(
    &self,
    storage_id: StorageId,
    format: ObjectFormatCode,
) -> Result<(), Error> {
    let response = self
        .execute(
            OperationCode::InitiateCapture,
            &[storage_id.0, format.to_code() as u32],
        )
        .await?;
    Self::check_response(response, OperationCode::InitiateCapture)?;
    Ok(())
}
```

**Tests to add**:

- Mock test for initiate_capture
- Test with various storage/format combinations

**Estimated complexity**: Small

**Dependencies**: Task 1.4

---

### Task 3.2: Add capture helper with event waiting

**Description**: Add a higher-level capture method that waits for completion.

**Files to modify**:

- `src/ptp/session.rs` - Add capture_and_wait method

**Implementation details**:

```rust
/// Capture an image and wait for completion.
///
/// This is a convenience method that:
/// 1. Initiates capture
/// 2. Waits for CaptureComplete event
/// 3. Collects ObjectAdded events for captured objects
///
/// # Returns
///
/// Returns handles of newly captured objects.
pub async fn capture_and_wait(
    &self,
    storage_id: StorageId,
    format: ObjectFormatCode,
    timeout: Duration,
) -> Result<Vec<ObjectHandle>, Error> {
    self.initiate_capture(storage_id, format).await?;

    let start = Instant::now();
    let mut new_objects = Vec::new();

    loop {
        if start.elapsed() > timeout {
            return Err(Error::Timeout);
        }

        match self.poll_event().await? {
            Some(event) if event.code == EventCode::CaptureComplete => {
                return Ok(new_objects);
            }
            Some(event) if event.code == EventCode::ObjectAdded => {
                new_objects.push(ObjectHandle(event.params[0]));
            }
            _ => {
                // Brief sleep to avoid busy-waiting
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}
```

Note: This uses tokio for the sleep. Consider making this method available only with a feature flag or use a
runtime-agnostic approach.

**Tests to add**:

- Mock test with queued events
- Test timeout behavior
- Test multiple object capture

**Estimated complexity**: Medium

**Dependencies**: Task 3.1, Task 1.5

---

## Phase 4: Testing and documentation

---

### Task 4.1: Add property parsing test fixtures

**Description**: Create test helper functions to build property descriptor bytes for testing.

**Files to modify**:

- `src/ptp/types.rs` (in `#[cfg(test)]` module)

**Implementation details**:

```rust
#[cfg(test)]
mod test_fixtures {
    /// Build a BatteryLevel property descriptor bytes.
    pub fn build_battery_level_prop_desc(current: u8) -> Vec<u8> {
        let mut buf = Vec::new();
        // PropertyCode: 0x5001
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
        buf.push(0);    // min
        buf.push(100);  // max
        buf.push(1);    // step
        buf
    }

    // Similar helpers for other property types...
}
```

**Estimated complexity**: Medium

**Dependencies**: Task 1.9

---

### Task 4.2: Add comprehensive unit tests

**Description**: Add thorough unit tests for all new functionality.

**Files to modify**:

- `src/ptp/codes.rs` - Tests in module
- `src/ptp/pack.rs` - Tests in module
- `src/ptp/types.rs` - Tests in module
- `src/ptp/session.rs` - Tests in module

**Test categories**:

1. Enum code roundtrips
2. Pack/unpack primitives
3. PropertyValue serialization
4. DevicePropDesc parsing
5. Session method mock tests

**Estimated complexity**: Large

**Dependencies**: All previous tasks

---

### Task 4.3: Add integration tests

**Description**: Add integration tests that work with real cameras (marked `#[ignore]`).

**Files to modify**:

- `tests/integration.rs` - Add `camera` module

**Tests to add**:

```rust
mod camera {
    /// Test reading battery level property.
    #[tokio::test]
    #[ignore]
    #[serial]
    async fn test_get_battery_level() { ... }

    /// Test reading supported properties.
    #[tokio::test]
    #[ignore]
    #[serial]
    async fn test_list_device_properties() { ... }

    /// Test setting a property (use StillCaptureMode or similar safe prop).
    #[tokio::test]
    #[ignore]
    #[serial]
    async fn test_set_device_property() { ... }

    /// Test capture (destructive - creates files on camera).
    #[tokio::test]
    #[ignore]
    #[serial]
    async fn test_initiate_capture() { ... }
}
```

**Estimated complexity**: Medium

**Dependencies**: All previous tasks

---

### Task 4.4: Update protocol documentation

**Description**: Update docs/protocol.md with property and capture operations.

**Files to modify**:

- `docs/protocol.md` - Add sections for new operations

**Sections to add**:

1. Device property operations (GetDevicePropDesc, etc.)
2. DevicePropDesc dataset format
3. Property data types
4. InitiateCapture operation
5. CaptureComplete event

**Estimated complexity**: Small

**Dependencies**: None (can be done in parallel)

---

### Task 4.5: Add module-level documentation

**Description**: Add comprehensive rustdoc documentation for new types and methods.

**Files to modify**:

- All modified files - Add/update doc comments

**Documentation to add**:

- Module-level docs explaining device properties and capture
- Type docs with examples
- Method docs with usage examples
- Error condition documentation

**Estimated complexity**: Medium

**Dependencies**: All implementation tasks

---

### Task 4.6: Update README and examples

**Description**: Add usage examples for new functionality.

**Files to modify**:

- `README.md` - Add camera control section
- `examples/` - Add camera_control.rs example

**Example to create**:

```rust
// examples/camera_control.rs
//! Example: Reading and setting camera properties

use mtp_rs::ptp::{PtpDevice, DevicePropertyCode, PropertyValue};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ... example code from plan document ...
}
```

**Estimated complexity**: Small

**Dependencies**: All implementation tasks

---

## Task dependency graph

```
Phase 1 (types and codes):
1.1 ─────────────────────────────────┐
1.2 ──────────────────────────┐      │
1.3 ────────────────────┐     │      │
1.4 ─────────────────┐  │     │      │
1.5 ──────────────┐  │  │     │      │
1.7 ───────────┐  │  │  │     │      │
               │  │  │  │     │      │
               ▼  │  │  │     ▼      ▼
1.6 (PropertyValue)◄──────────┴──────┘
               │
               ▼
1.8 (PropertyRange)
               │
               ▼
1.9 (DevicePropDesc)◄────┬────────────
               │         │
Phase 2:       ▼         │
2.1 (get_prop_desc)      │
2.2 (get_prop_value)     │
2.3 (set_prop_value)     │
2.4 (reset_prop)         │
               │         │
               ▼         │
2.5 (typed helpers)      │
                         │
Phase 3:                 │
3.1 (initiate_capture)◄──┤
               │         │
               ▼         │
3.2 (capture_and_wait)   │
                         │
Phase 4:                 ▼
4.1 (fixtures)───────► 4.2 (unit tests)
                         │
                         ▼
4.3 (integration tests)
4.4 (protocol docs) ─────┐
4.5 (rustdoc) ───────────┼──► 4.6 (README/examples)
                         │
```

## Estimated total effort

| Phase   | Tasks   | Complexity                 |
|---------|---------|----------------------------|
| Phase 1 | 9 tasks | 2 large, 1 medium, 6 small |
| Phase 2 | 5 tasks | 1 medium, 4 small          |
| Phase 3 | 2 tasks | 1 medium, 1 small          |
| Phase 4 | 6 tasks | 1 large, 3 medium, 2 small |

**Total: 22 tasks**

- Small: 13
- Medium: 6
- Large: 3

## Critical files for implementation

| File                 | Changes                                                                         |
|----------------------|---------------------------------------------------------------------------------|
| `src/ptp/codes.rs`   | Add DevicePropertyCode, PropertyDataType enums and new operation/response codes |
| `src/ptp/types.rs`   | Add DevicePropDesc, PropertyValue, PropertyRange structs with parsing logic     |
| `src/ptp/session.rs` | Add property and capture methods to PtpSession                                  |
| `src/ptp/pack.rs`    | Add signed integer pack/unpack functions (i8, i16, i32, i64)                    |
| `src/ptp/mod.rs`     | Export new public types                                                         |
