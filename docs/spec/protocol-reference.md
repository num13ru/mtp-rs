# MTP/PTP Protocol Reference

This document extracts the essential protocol details needed for implementing `mtp-rs`. For the full specification, see
the MTP v1.1 spec in `docs/mtp-v1_1-spec/`.

---

## Byte Order

All multi-byte values in MTP/PTP are **little-endian**.

```rust
// Encoding a u32 value 0x12345678:
[0x78, 0x56, 0x34, 0x12]  // LSB first
```

---

## Simple Types

| Type Code | Name   | Size (bytes) | Rust Type |
|-----------|--------|--------------|-----------|
| 0x0001    | INT8   | 1            | `i8`      |
| 0x0002    | UINT8  | 1            | `u8`      |
| 0x0003    | INT16  | 2            | `i16`     |
| 0x0004    | UINT16 | 2            | `u16`     |
| 0x0005    | INT32  | 4            | `i32`     |
| 0x0006    | UINT32 | 4            | `u32`     |
| 0x0007    | INT64  | 8            | `i64`     |
| 0x0008    | UINT64 | 8            | `u64`     |
| 0xFFFF    | STR    | Variable     | `String`  |

---

## Strings

Strings are encoded as:

1. **1 byte**: Number of characters (including null terminator)
2. **N × 2 bytes**: UTF-16LE encoded characters
3. **2 bytes**: Null terminator (0x0000)

**Empty string**: Single byte `0x00`

**Example**: String "Hi"

```
03          // Length: 3 chars (H, i, null)
48 00       // 'H' in UTF-16LE
69 00       // 'i' in UTF-16LE
00 00       // Null terminator
```

**Maximum length**: 255 characters (including null)

```rust
fn pack_string(s: &str) -> Vec<u8> {
    if s.is_empty() {
        return vec![0x00];
    }
    let utf16: Vec<u16> = s.encode_utf16().collect();
    let len = (utf16.len() + 1) as u8;  // +1 for null terminator
    let mut buf = vec![len];
    for c in utf16 {
        buf.extend_from_slice(&c.to_le_bytes());
    }
    buf.extend_from_slice(&[0x00, 0x00]);  // Null terminator
    buf
}
```

---

## Arrays

Arrays are encoded as:

1. **4 bytes**: Element count (u32, little-endian)
2. **N × element_size**: Elements

**Empty array**: `00 00 00 00`

**Example**: Array of three u32 values [1, 2, 3]

```
03 00 00 00    // Count: 3
01 00 00 00    // Element 0: 1
02 00 00 00    // Element 1: 2
03 00 00 00    // Element 2: 3
```

```rust
fn pack_u32_array(arr: &[u32]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + arr.len() * 4);
    buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
    for &val in arr {
        buf.extend_from_slice(&val.to_le_bytes());
    }
    buf
}
```

---

## USB Container Format

All MTP/PTP communication uses containers. The USB SIC (Still Image Capture) transport uses this format:

### Container Header (12 bytes)

| Offset | Size | Field         | Description                             |
|--------|------|---------------|-----------------------------------------|
| 0      | 4    | Length        | Total container size (header + payload) |
| 4      | 2    | Type          | Container type (see below)              |
| 6      | 2    | Code          | Operation/Response/Event code           |
| 8      | 4    | TransactionID | Transaction identifier                  |

### Container Types

| Value  | Name     | Direction                 |
|--------|----------|---------------------------|
| 0x0001 | Command  | Host → Device             |
| 0x0002 | Data     | Either direction          |
| 0x0003 | Response | Device → Host             |
| 0x0004 | Event    | Device → Host (interrupt) |

### Command Container

```
┌────────────────────────────────────────────────────┐
│ Length (4)  │ Type=1 (2) │ OpCode (2) │ TxID (4)  │
├────────────────────────────────────────────────────┤
│ Param1 (4)  │ Param2 (4) │ Param3 (4) │ ...       │
└────────────────────────────────────────────────────┘
```

- Up to 5 parameters (each 4 bytes)
- Length = 12 + (num_params × 4)

**Example**: OpenSession with SessionID=1

```
10 00 00 00    // Length: 16 bytes
01 00          // Type: Command
02 10          // Code: 0x1002 (OpenSession)
01 00 00 00    // TransactionID: 1
01 00 00 00    // Param1: SessionID = 1
```

### Data Container

```
┌────────────────────────────────────────────────────┐
│ Length (4)  │ Type=2 (2) │ OpCode (2) │ TxID (4)  │
├────────────────────────────────────────────────────┤
│                    Payload...                      │
└────────────────────────────────────────────────────┘
```

- Echoes the operation code from command
- Same transaction ID
- Large payloads split across multiple USB packets

### Response Container

```
┌────────────────────────────────────────────────────┐
│ Length (4)  │ Type=3 (2) │ RespCode (2)│ TxID (4) │
├────────────────────────────────────────────────────┤
│ Param1 (4)  │ Param2 (4) │ ...                    │
└────────────────────────────────────────────────────┘
```

- Contains response code
- May include up to 5 response parameters

### Event Container

```
┌────────────────────────────────────────────────────┐
│ Length (4)  │ Type=4 (2) │ EvtCode (2)│ TxID (4)  │
├────────────────────────────────────────────────────┤
│ Param1 (4)  │ Param2 (4) │ Param3 (4)            │
└────────────────────────────────────────────────────┘
```

- Sent on interrupt endpoint
- Up to 3 parameters
- Can arrive during any phase

---

## Transaction Flow

### Basic Operation (no data phase)

```
Host                          Device
  │                              │
  │─── Command Container ───────▶│
  │                              │
  │◀── Response Container ───────│
  │                              │
```

### Operation with Data (Device → Host)

```
Host                          Device
  │                              │
  │─── Command Container ───────▶│
  │                              │
  │◀── Data Container(s) ────────│
  │                              │
  │◀── Response Container ───────│
  │                              │
```

### Operation with Data (Host → Device)

```
Host                          Device
  │                              │
  │─── Command Container ───────▶│
  │                              │
  │─── Data Container(s) ───────▶│
  │                              │
  │◀── Response Container ───────│
  │                              │
```

---

## Transaction IDs

- Start at 0x00000001 for first transaction in session
- Increment by 1 for each transaction
- Wrap from 0xFFFFFFFE to 0x00000001 (skip both 0xFFFFFFFF and 0x00000000)
- 0x00000000 used only for session-less operations (GetDeviceInfo before OpenSession)
- 0xFFFFFFFF is invalid and must never be used

**Correct sequence**: `...0xFFFFFFFD, 0xFFFFFFFE, 0x00000001, 0x00000002...`

---

## Sessions

- Most operations require an open session
- Session opened with `OpenSession` operation
- Session ID chosen by host (usually 1)
- Only `GetDeviceInfo` works without a session
- Most devices support only one session

---

## Operations We Need

### GetDeviceInfo (0x1001)

**Parameters**: None
**Data Phase**: R→I (DeviceInfo dataset)
**No session required**

Returns device capabilities and identification.

### OpenSession (0x1002)

**Parameters**:

- Param1: SessionID (chosen by initiator, usually 1)

**Data Phase**: None

Opens a session. Most operations require this first.

### CloseSession (0x1003)

**Parameters**: None
**Data Phase**: None

Closes the current session.

### GetStorageIDs (0x1004)

**Parameters**: None
**Data Phase**: R→I (array of u32 StorageIDs)

Returns list of storage IDs on device.

### GetStorageInfo (0x1005)

**Parameters**:

- Param1: StorageID

**Data Phase**: R→I (StorageInfo dataset)

Returns information about a specific storage.

### GetObjectHandles (0x1007)

**Parameters**:

- Param1: StorageID (0xFFFFFFFF = all storages)
- Param2: ObjectFormatCode (0x00000000 = all formats)
- Param3: Parent ObjectHandle:
    - 0x00000000 = objects in root folder only
    - 0xFFFFFFFF = all objects recursively (all folders)
    - Other = objects in that specific folder

**Data Phase**: R→I (array of u32 ObjectHandles)

Returns list of objects matching criteria.

### GetObjectInfo (0x1008)

**Parameters**:

- Param1: ObjectHandle

**Data Phase**: R→I (ObjectInfo dataset)

Returns metadata for an object.

### GetObject (0x1009)

**Parameters**:

- Param1: ObjectHandle

**Data Phase**: R→I (object binary data)

Downloads object content.

### GetThumb (0x100A)

**Parameters**:

- Param1: ObjectHandle

**Data Phase**: R→I (thumbnail data)

Downloads thumbnail for an object.

### DeleteObject (0x100B)

**Parameters**:

- Param1: ObjectHandle
- Param2: ObjectFormatCode (0x00000000 to delete regardless of format)

**Data Phase**: None

Deletes an object.

### SendObjectInfo (0x100C)

**Parameters**:

- Param1: StorageID (0xFFFFFFFF = let device choose)
- Param2: Parent ObjectHandle (0x00000000 = root folder)

**Data Phase**: I→R (ObjectInfo dataset)

**Response Parameters**:

- Param1: StorageID (assigned)
- Param2: Parent ObjectHandle (assigned)
- Param3: ObjectHandle (assigned to new object)

Prepares device to receive an object. Must be followed by SendObject.

### SendObject (0x100D)

**Parameters**: None
**Data Phase**: I→R (object binary data)

Sends object content. Must follow SendObjectInfo.

### MoveObject (0x1019)

**Parameters**:

- Param1: ObjectHandle
- Param2: StorageID (destination)
- Param3: Parent ObjectHandle (destination folder)

**Data Phase**: None

Moves an object to a different location.

### CopyObject (0x101A)

**Parameters**:

- Param1: ObjectHandle
- Param2: StorageID (destination)
- Param3: Parent ObjectHandle (destination folder)

**Data Phase**: None

**Response Parameters**:

- Param1: New ObjectHandle

Copies an object.

### GetPartialObject (0x101B)

**Parameters**:

- Param1: ObjectHandle
- Param2: Offset (bytes from start)
- Param3: MaxBytes (maximum bytes to return)

**Data Phase**: R→I (partial object data)

**Response Parameters**:

- Param1: ActualBytes (bytes actually returned)

Downloads a portion of an object.

---

## Response Codes

| Code   | Name                    | Description                      |
|--------|-------------------------|----------------------------------|
| 0x2001 | OK                      | Success                          |
| 0x2002 | General_Error           | Unknown error                    |
| 0x2003 | Session_Not_Open        | Session required but not open    |
| 0x2004 | Invalid_TransactionID   | Bad transaction ID               |
| 0x2005 | Operation_Not_Supported | Device doesn't support operation |
| 0x2006 | Parameter_Not_Supported | Unexpected parameter value       |
| 0x2007 | Incomplete_Transfer     | Transfer didn't complete         |
| 0x2008 | Invalid_StorageID       | Storage doesn't exist            |
| 0x2009 | Invalid_ObjectHandle    | Object doesn't exist             |
| 0x200C | Store_Full              | No space left                    |
| 0x200D | Object_WriteProtected   | Can't modify protected object    |
| 0x200E | Store_Read-Only         | Storage is read-only             |
| 0x200F | Access_Denied           | Permission denied                |
| 0x2010 | No_Thumbnail_Present    | Object has no thumbnail          |
| 0x2019 | Device_Busy             | Try again later                  |
| 0x201A | Invalid_ParentObject    | Parent isn't a folder            |
| 0x201D | Invalid_Parameter       | Bad parameter value              |
| 0x201E | Session_Already_Open    | Session already exists           |
| 0x201F | Transaction_Cancelled   | Operation was cancelled          |
| 0xA809 | Object_Too_Large        | File too large for filesystem    |

---

## Event Codes

| Code   | Name               | Parameters   | Description             |
|--------|--------------------|--------------|-------------------------|
| 0x4002 | ObjectAdded        | ObjectHandle | New object created      |
| 0x4003 | ObjectRemoved      | ObjectHandle | Object deleted          |
| 0x4004 | StoreAdded         | StorageID    | Storage mounted         |
| 0x4005 | StoreRemoved       | StorageID    | Storage unmounted       |
| 0x4006 | DevicePropChanged  | PropCode     | Device property changed |
| 0x4007 | ObjectInfoChanged  | ObjectHandle | Object metadata changed |
| 0x4008 | DeviceInfoChanged  | (none)       | Device info changed     |
| 0x400C | StorageInfoChanged | StorageID    | Storage info changed    |

---

## Data Structures

### DeviceInfo Dataset

| Field                     | Type       | Description                                |
|---------------------------|------------|--------------------------------------------|
| StandardVersion           | u16        | PTP version × 100 (100 = 1.00)             |
| VendorExtensionID         | u32        | 0xFFFFFFFF for MTP                         |
| VendorExtensionVersion    | u16        | MTP version × 100                          |
| VendorExtensionDesc       | String     | Extension info (e.g., "android.com: 1.0;") |
| FunctionalMode            | u16        | 0x0000 = standard mode                     |
| OperationsSupported       | Array<u16> | Supported operation codes                  |
| EventsSupported           | Array<u16> | Supported event codes                      |
| DevicePropertiesSupported | Array<u16> | Supported device property codes            |
| CaptureFormats            | Array<u16> | Formats device can create                  |
| PlaybackFormats           | Array<u16> | Formats device can read                    |
| Manufacturer              | String     | Device manufacturer                        |
| Model                     | String     | Device model                               |
| DeviceVersion             | String     | Firmware version                           |
| SerialNumber              | String     | Unique identifier                          |

### StorageInfo Dataset

| Field              | Type   | Description                                |
|--------------------|--------|--------------------------------------------|
| StorageType        | u16    | 0x0003 = Fixed RAM, 0x0004 = Removable RAM |
| FilesystemType     | u16    | 0x0002 = Generic hierarchical              |
| AccessCapability   | u16    | 0x0000 = Read-write                        |
| MaxCapacity        | u64    | Total capacity in bytes                    |
| FreeSpaceInBytes   | u64    | Available space                            |
| FreeSpaceInObjects | u32    | 0xFFFFFFFF if not applicable               |
| StorageDescription | String | Human-readable name                        |
| VolumeIdentifier   | String | Volume serial number                       |

### ObjectInfo Dataset

| Field                | Type   | Required for Send | Description                    |
|----------------------|--------|-------------------|--------------------------------|
| StorageID            | u32    | No                | Storage containing object      |
| ObjectFormat         | u16    | Yes               | Object type code               |
| ProtectionStatus     | u16    | No                | Write protection               |
| ObjectCompressedSize | u32    | Yes               | File size (0xFFFFFFFF if >4GB) |
| ThumbFormat          | u16    | No                | Thumbnail format               |
| ThumbCompressedSize  | u32    | No                | Thumbnail size                 |
| ThumbPixWidth        | u32    | No                | Thumbnail width                |
| ThumbPixHeight       | u32    | No                | Thumbnail height               |
| ImagePixWidth        | u32    | No                | Image width                    |
| ImagePixHeight       | u32    | No                | Image height                   |
| ImageBitDepth        | u32    | No                | Bits per pixel                 |
| ParentObject         | u32    | No                | Parent folder handle           |
| AssociationType      | u16    | Yes               | 0x0001 for folders             |
| AssociationDesc      | u32    | Yes               | 0x00000000 normally            |
| SequenceNumber       | u32    | No                | Unused                         |
| Filename             | String | Yes               | Object name                    |
| DateCreated          | String | No                | "YYYYMMDDThhmmss"              |
| DateModified         | String | No                | "YYYYMMDDThhmmss"              |
| Keywords             | String | No                | Unused                         |

### DateTime Format

ISO 8601 subset: `YYYYMMDDThhmmss`

Example: `20240115T143022` = January 15, 2024 at 14:30:22

Optional timezone suffix: `Z` (UTC) or `+hhmm`/`-hhmm`

---

## Object Format Codes

| Code   | Format      | Description         |
|--------|-------------|---------------------|
| 0x3000 | Undefined   | Unknown binary      |
| 0x3001 | Association | Folder              |
| 0x3004 | Text        | Plain text          |
| 0x3005 | HTML        | HTML document       |
| 0x3008 | WAV         | WAV audio           |
| 0x3009 | MP3         | MP3 audio           |
| 0x300B | MPEG        | MPEG video          |
| 0x3801 | JPEG        | JPEG image          |
| 0x3804 | TIFF        | TIFF image          |
| 0x3807 | GIF         | GIF image           |
| 0x380B | PNG         | PNG image           |
| 0xB901 | WMA         | Windows Media Audio |
| 0xB902 | OGG         | Ogg Vorbis          |
| 0xB903 | AAC         | AAC audio           |
| 0xB906 | FLAC        | FLAC audio          |
| 0xB982 | MP4         | MP4 container       |
| 0xB984 | M4A         | M4A audio           |
| 0xB981 | WMV         | Windows Media Video |

---

## USB Endpoints

MTP devices expose three endpoints:

| Endpoint     | Type      | Direction     | Purpose                        |
|--------------|-----------|---------------|--------------------------------|
| Bulk OUT     | Bulk      | Host → Device | Commands and data to device    |
| Bulk IN      | Bulk      | Device → Host | Responses and data from device |
| Interrupt IN | Interrupt | Device → Host | Events                         |

Typical max packet sizes:

- USB 2.0 High Speed: 512 bytes
- USB 3.0 Super Speed: 1024 bytes

---

## Android-Specific Behavior

All modern Android devices:

- Report "android.com" in VendorExtensionDesc
- Support hierarchical filesystem (AssociationType = 0x0001)
- May have broken GetObjectPropList (use GetObjectInfo instead)
- Need 30+ second timeouts for large transfers
- Support events on interrupt endpoint

No per-device quirks database needed for modern Android.
