# Test Cases

This document defines specific test cases for `mtp-rs`. Tests are organized by module and phase.

---

## Testing Philosophy

1. **Unit tests first**: Every serialization function, every parser
2. **Mock transport for protocol logic**: No USB dependency for most tests
3. **Property-based testing**: Catch edge cases automatically
4. **Integration tests last**: Real device only for final validation

---

## Phase 1: Serialization Tests

### Primitive Types (`ptp/pack.rs`)

```rust
mod pack_tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════
    // INTEGERS
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn pack_u8() {
        assert_eq!(pack_u8(0x00), [0x00]);
        assert_eq!(pack_u8(0xFF), [0xFF]);
        assert_eq!(pack_u8(0x42), [0x42]);
    }

    #[test]
    fn pack_u16_little_endian() {
        assert_eq!(pack_u16(0x0000), [0x00, 0x00]);
        assert_eq!(pack_u16(0xFFFF), [0xFF, 0xFF]);
        assert_eq!(pack_u16(0x1234), [0x34, 0x12]);
        assert_eq!(pack_u16(0x0001), [0x01, 0x00]);
    }

    #[test]
    fn pack_u32_little_endian() {
        assert_eq!(pack_u32(0x00000000), [0x00, 0x00, 0x00, 0x00]);
        assert_eq!(pack_u32(0xFFFFFFFF), [0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(pack_u32(0x12345678), [0x78, 0x56, 0x34, 0x12]);
        assert_eq!(pack_u32(0x00000001), [0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn pack_u64_little_endian() {
        assert_eq!(pack_u64(0x0102030405060708),
                   [0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn unpack_u16() {
        assert_eq!(unpack_u16(&[0x34, 0x12]).unwrap(), 0x1234);
    }

    #[test]
    fn unpack_u16_insufficient_bytes() {
        assert!(unpack_u16(&[0x34]).is_err());
        assert!(unpack_u16(&[]).is_err());
    }

    #[test]
    fn unpack_u32() {
        assert_eq!(unpack_u32(&[0x78, 0x56, 0x34, 0x12]).unwrap(), 0x12345678);
    }

    // ═══════════════════════════════════════════════════════════
    // STRINGS
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn pack_string_empty() {
        assert_eq!(pack_string(""), vec![0x00]);
    }

    #[test]
    fn pack_string_single_char() {
        // "A" -> length=2 (char + null), then UTF-16LE
        assert_eq!(pack_string("A"), vec![
            0x02,             // length = 2
            0x41, 0x00,       // 'A'
            0x00, 0x00,       // null
        ]);
    }

    #[test]
    fn pack_string_ascii() {
        assert_eq!(pack_string("Hi"), vec![
            0x03,             // length = 3
            0x48, 0x00,       // 'H'
            0x69, 0x00,       // 'i'
            0x00, 0x00,       // null
        ]);
    }

    #[test]
    fn pack_string_unicode() {
        // "日" (U+65E5) -> 0xE5 0x65 in UTF-16LE
        let packed = pack_string("日");
        assert_eq!(packed[0], 2);  // length
        assert_eq!(&packed[1..3], &[0xE5, 0x65]);
    }

    #[test]
    fn pack_string_emoji() {
        // "😀" (U+1F600) is a surrogate pair in UTF-16
        let packed = pack_string("😀");
        assert_eq!(packed[0], 3);  // 2 UTF-16 units + null
    }

    #[test]
    fn unpack_string_empty() {
        let (s, len) = unpack_string(&[0x00]).unwrap();
        assert_eq!(s, "");
        assert_eq!(len, 1);
    }

    #[test]
    fn unpack_string_ascii() {
        let data = vec![0x03, 0x48, 0x00, 0x69, 0x00, 0x00, 0x00];
        let (s, len) = unpack_string(&data).unwrap();
        assert_eq!(s, "Hi");
        assert_eq!(len, 7);
    }

    #[test]
    fn string_roundtrip() {
        let test_cases = vec![
            "",
            "A",
            "Hello World",
            "日本語",
            "emoji 😀 test",
            "path/to/file.mp3",
            "spaces   and\ttabs",
        ];

        for original in test_cases {
            let packed = pack_string(original);
            let (unpacked, _) = unpack_string(&packed).unwrap();
            assert_eq!(unpacked, original, "Failed for: {:?}", original);
        }
    }

    // ═══════════════════════════════════════════════════════════
    // ARRAYS
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn pack_u32_array_empty() {
        assert_eq!(pack_u32_array(&[]), vec![0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn pack_u32_array_single() {
        assert_eq!(pack_u32_array(&[0x12345678]), vec![
            0x01, 0x00, 0x00, 0x00,  // count = 1
            0x78, 0x56, 0x34, 0x12,  // element
        ]);
    }

    #[test]
    fn pack_u32_array_multiple() {
        assert_eq!(pack_u32_array(&[1, 2, 3]), vec![
            0x03, 0x00, 0x00, 0x00,  // count = 3
            0x01, 0x00, 0x00, 0x00,  // 1
            0x02, 0x00, 0x00, 0x00,  // 2
            0x03, 0x00, 0x00, 0x00,  // 3
        ]);
    }

    #[test]
    fn pack_u16_array() {
        assert_eq!(pack_u16_array(&[0x1001, 0x1002]), vec![
            0x02, 0x00, 0x00, 0x00,  // count = 2
            0x01, 0x10,              // 0x1001
            0x02, 0x10,              // 0x1002
        ]);
    }

    #[test]
    fn unpack_u32_array() {
        let data = vec![
            0x02, 0x00, 0x00, 0x00,
            0x01, 0x00, 0x01, 0x00,
            0x02, 0x00, 0x01, 0x00,
        ];
        let (arr, len) = unpack_u32_array(&data).unwrap();
        assert_eq!(arr, vec![0x00010001, 0x00010002]);
        assert_eq!(len, 12);
    }

    #[test]
    fn array_roundtrip() {
        let test_cases: Vec<Vec<u32>> = vec![
            vec![],
            vec![1],
            vec![1, 2, 3],
            vec![0, u32::MAX, 0x12345678],
            (0..100).collect(),
        ];

        for original in test_cases {
            let packed = pack_u32_array(&original);
            let (unpacked, _) = unpack_u32_array(&packed).unwrap();
            assert_eq!(unpacked, original);
        }
    }

    // ═══════════════════════════════════════════════════════════
    // DATETIME
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn pack_datetime() {
        let dt = DateTime {
            year: 2024,
            month: 1,
            day: 15,
            hour: 14,
            minute: 30,
            second: 22,
        };
        let packed = pack_datetime(&dt);
        let (unpacked, _) = unpack_datetime(&packed).unwrap();
        assert_eq!(unpacked, Some(dt));
    }

    #[test]
    fn unpack_datetime_with_timezone() {
        // "20240115T143022Z" - UTC timezone (ignored, stored as naive)
        let packed = pack_string("20240115T143022Z");
        let (dt, _) = unpack_datetime(&packed).unwrap();
        assert_eq!(dt.unwrap().year, 2024);
        // Timezone suffix is parsed but not stored
    }

    #[test]
    fn unpack_datetime_empty() {
        let packed = pack_string("");
        let (dt, _) = unpack_datetime(&packed).unwrap();
        assert_eq!(dt, None);
    }

    #[test]
    fn datetime_roundtrip() {
        let dt = DateTime {
            year: 2024,
            month: 12,
            day: 31,
            hour: 23,
            minute: 59,
            second: 59,
        };
        let packed = pack_datetime(&dt);
        let (unpacked, _) = unpack_datetime(&packed).unwrap();
        assert_eq!(unpacked, Some(dt));
    }
}
```

### Property-Based Tests

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn u16_roundtrip(val: u16) {
        let packed = pack_u16(val);
        let unpacked = unpack_u16(&packed).unwrap();
        prop_assert_eq!(val, unpacked);
    }

    #[test]
    fn u32_roundtrip(val: u32) {
        let packed = pack_u32(val);
        let unpacked = unpack_u32(&packed).unwrap();
        prop_assert_eq!(val, unpacked);
    }

    #[test]
    fn string_roundtrip_proptest(s in "\\PC{0,200}") {
        let packed = pack_string(&s);
        let (unpacked, _) = unpack_string(&packed).unwrap();
        prop_assert_eq!(s, unpacked);
    }

    #[test]
    fn u32_array_roundtrip_proptest(arr in prop::collection::vec(any::<u32>(), 0..100)) {
        let packed = pack_u32_array(&arr);
        let (unpacked, _) = unpack_u32_array(&packed).unwrap();
        prop_assert_eq!(arr, unpacked);
    }
}
```

---

## Phase 2: Container Tests

### Command Container (`ptp/container.rs`)

```rust
mod container_tests {
    #[test]
    fn command_container_no_params() {
        let cmd = CommandContainer {
            code: OperationCode::CloseSession,
            transaction_id: 5,
            params: vec![],
        };

        let bytes = cmd.to_bytes();

        assert_eq!(bytes.len(), 12);
        assert_eq!(&bytes[0..4], &[0x0C, 0x00, 0x00, 0x00]);  // length = 12
        assert_eq!(&bytes[4..6], &[0x01, 0x00]);              // type = Command
        assert_eq!(&bytes[6..8], &[0x03, 0x10]);              // code = CloseSession
        assert_eq!(&bytes[8..12], &[0x05, 0x00, 0x00, 0x00]); // transaction_id = 5
    }

    #[test]
    fn command_container_one_param() {
        let cmd = CommandContainer {
            code: OperationCode::OpenSession,
            transaction_id: 1,
            params: vec![1],
        };

        let bytes = cmd.to_bytes();

        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[0..4], &[0x10, 0x00, 0x00, 0x00]);  // length = 16
        assert_eq!(&bytes[12..16], &[0x01, 0x00, 0x00, 0x00]); // param1 = 1
    }

    #[test]
    fn command_container_five_params() {
        let cmd = CommandContainer {
            code: OperationCode::GetObjectHandles,
            transaction_id: 10,
            params: vec![0xFFFFFFFF, 0, 0, 0, 0],
        };

        let bytes = cmd.to_bytes();
        assert_eq!(bytes.len(), 32);  // 12 + 5*4
    }

    #[test]
    fn response_container_ok() {
        let bytes = vec![
            0x0C, 0x00, 0x00, 0x00,  // length = 12
            0x03, 0x00,              // type = Response
            0x01, 0x20,              // code = OK (0x2001)
            0x01, 0x00, 0x00, 0x00,  // transaction_id = 1
        ];

        let resp = ResponseContainer::from_bytes(&bytes).unwrap();

        assert_eq!(resp.code, ResponseCode::Ok);
        assert_eq!(resp.transaction_id, 1);
        assert!(resp.params.is_empty());
    }

    #[test]
    fn response_container_with_params() {
        let bytes = vec![
            0x18, 0x00, 0x00, 0x00,  // length = 24
            0x03, 0x00,              // type = Response
            0x01, 0x20,              // code = OK
            0x02, 0x00, 0x00, 0x00,  // transaction_id = 2
            0x01, 0x00, 0x01, 0x00,  // param1 = StorageID
            0x00, 0x00, 0x00, 0x00,  // param2 = ParentHandle
            0x05, 0x00, 0x00, 0x00,  // param3 = ObjectHandle
        ];

        let resp = ResponseContainer::from_bytes(&bytes).unwrap();

        assert_eq!(resp.params.len(), 3);
        assert_eq!(resp.params[0], 0x00010001);
        assert_eq!(resp.params[2], 5);
    }

    #[test]
    fn response_container_error() {
        let bytes = vec![
            0x0C, 0x00, 0x00, 0x00,
            0x03, 0x00,
            0x09, 0x20,              // Invalid_ObjectHandle
            0x05, 0x00, 0x00, 0x00,
        ];

        let resp = ResponseContainer::from_bytes(&bytes).unwrap();
        assert_eq!(resp.code, ResponseCode::InvalidObjectHandle);
    }

    #[test]
    fn data_container_parse() {
        let bytes = vec![
            0x14, 0x00, 0x00, 0x00,  // length = 20
            0x02, 0x00,              // type = Data
            0x04, 0x10,              // code = GetStorageIDs
            0x02, 0x00, 0x00, 0x00,  // transaction_id = 2
            // payload: array with 1 element
            0x01, 0x00, 0x00, 0x00,
            0x01, 0x00, 0x01, 0x00,
        ];

        let data = DataContainer::from_bytes(&bytes).unwrap();

        assert_eq!(data.code, OperationCode::GetStorageIds);
        assert_eq!(data.payload.len(), 8);
    }

    #[test]
    fn event_container_object_added() {
        let bytes = vec![
            0x18, 0x00, 0x00, 0x00,  // length = 24
            0x04, 0x00,              // type = Event
            0x02, 0x40,              // code = ObjectAdded (0x4002)
            0x00, 0x00, 0x00, 0x00,  // transaction_id
            0x0A, 0x00, 0x00, 0x00,  // param1 = ObjectHandle 10
            0x00, 0x00, 0x00, 0x00,  // param2
            0x00, 0x00, 0x00, 0x00,  // param3
        ];

        let event = EventContainer::from_bytes(&bytes).unwrap();

        assert_eq!(event.code, EventCode::ObjectAdded);
        assert_eq!(event.params[0], 10);
    }

    #[test]
    fn container_type_detection() {
        let command = vec![0x0C, 0x00, 0x00, 0x00, 0x01, 0x00, /* ... */];
        let data = vec![0x0C, 0x00, 0x00, 0x00, 0x02, 0x00, /* ... */];
        let response = vec![0x0C, 0x00, 0x00, 0x00, 0x03, 0x00, /* ... */];
        let event = vec![0x0C, 0x00, 0x00, 0x00, 0x04, 0x00, /* ... */];

        assert_eq!(container_type(&command), ContainerType::Command);
        assert_eq!(container_type(&data), ContainerType::Data);
        assert_eq!(container_type(&response), ContainerType::Response);
        assert_eq!(container_type(&event), ContainerType::Event);
    }
}
```

---

## Phase 2: Data Structure Tests

### DeviceInfo (`ptp/types.rs`)

```rust
mod device_info_tests {
    #[test]
    fn parse_minimal_device_info() {
        // Minimal valid DeviceInfo
        let mut data = Vec::new();
        data.extend_from_slice(&pack_u16(100));           // StandardVersion
        data.extend_from_slice(&pack_u32(0xFFFFFFFF));    // VendorExtensionID
        data.extend_from_slice(&pack_u16(100));           // VendorExtensionVersion
        data.extend_from_slice(&pack_string(""));         // VendorExtensionDesc
        data.extend_from_slice(&pack_u16(0));             // FunctionalMode
        data.extend_from_slice(&pack_u16_array(&[]));     // OperationsSupported
        data.extend_from_slice(&pack_u16_array(&[]));     // EventsSupported
        data.extend_from_slice(&pack_u16_array(&[]));     // DevicePropertiesSupported
        data.extend_from_slice(&pack_u16_array(&[]));     // CaptureFormats
        data.extend_from_slice(&pack_u16_array(&[]));     // PlaybackFormats
        data.extend_from_slice(&pack_string("Test"));     // Manufacturer
        data.extend_from_slice(&pack_string("Device"));   // Model
        data.extend_from_slice(&pack_string("1.0"));      // DeviceVersion
        data.extend_from_slice(&pack_string("12345678")); // SerialNumber

        let info = DeviceInfo::from_bytes(&data).unwrap();

        assert_eq!(info.manufacturer, "Test");
        assert_eq!(info.model, "Device");
        assert_eq!(info.serial_number, "12345678");
    }

    #[test]
    fn parse_android_device_info() {
        // Typical Android device
        let mut data = Vec::new();
        data.extend_from_slice(&pack_u16(100));
        data.extend_from_slice(&pack_u32(0xFFFFFFFF));
        data.extend_from_slice(&pack_u16(100));
        data.extend_from_slice(&pack_string("android.com: 1.0;"));
        data.extend_from_slice(&pack_u16(0));
        data.extend_from_slice(&pack_u16_array(&[0x1001, 0x1002, 0x1004, 0x1007]));
        // ... rest of fields

        let info = DeviceInfo::from_bytes(&data).unwrap();

        assert!(info.vendor_extension_desc.contains("android.com"));
        assert!(info.operations_supported.contains(&OperationCode::GetDeviceInfo));
    }
}
```

### StorageInfo

```rust
mod storage_info_tests {
    #[test]
    fn parse_storage_info() {
        let mut data = Vec::new();
        data.extend_from_slice(&pack_u16(0x0003));        // StorageType = Fixed RAM
        data.extend_from_slice(&pack_u16(0x0002));        // FilesystemType = Hierarchical
        data.extend_from_slice(&pack_u16(0x0000));        // AccessCapability = ReadWrite
        data.extend_from_slice(&pack_u64(64_000_000_000)); // MaxCapacity = 64GB
        data.extend_from_slice(&pack_u64(32_000_000_000)); // FreeSpace = 32GB
        data.extend_from_slice(&pack_u32(0xFFFFFFFF));    // FreeSpaceInObjects = N/A
        data.extend_from_slice(&pack_string("Internal storage"));
        data.extend_from_slice(&pack_string(""));

        let info = StorageInfo::from_bytes(&data).unwrap();

        assert_eq!(info.storage_type, StorageType::FixedRam);
        assert_eq!(info.max_capacity, 64_000_000_000);
        assert_eq!(info.free_space_bytes, 32_000_000_000);
        assert_eq!(info.description, "Internal storage");
    }
}
```

### ObjectInfo

```rust
mod object_info_tests {
    #[test]
    fn parse_file_object_info() {
        let mut data = Vec::new();
        data.extend_from_slice(&pack_u32(0x00010001));    // StorageID
        data.extend_from_slice(&pack_u16(0x3009));        // ObjectFormat = MP3
        data.extend_from_slice(&pack_u16(0x0000));        // ProtectionStatus
        data.extend_from_slice(&pack_u32(5_000_000));     // CompressedSize = 5MB
        data.extend_from_slice(&pack_u16(0x0000));        // ThumbFormat
        data.extend_from_slice(&pack_u32(0));             // ThumbCompressedSize
        data.extend_from_slice(&pack_u32(0));             // ThumbPixWidth
        data.extend_from_slice(&pack_u32(0));             // ThumbPixHeight
        data.extend_from_slice(&pack_u32(0));             // ImagePixWidth
        data.extend_from_slice(&pack_u32(0));             // ImagePixHeight
        data.extend_from_slice(&pack_u32(0));             // ImageBitDepth
        data.extend_from_slice(&pack_u32(5));             // ParentObject
        data.extend_from_slice(&pack_u16(0x0000));        // AssociationType
        data.extend_from_slice(&pack_u32(0));             // AssociationDesc
        data.extend_from_slice(&pack_u32(0));             // SequenceNumber
        data.extend_from_slice(&pack_string("song.mp3"));
        data.extend_from_slice(&pack_string("20240115T143022"));
        data.extend_from_slice(&pack_string("20240115T143022"));
        data.extend_from_slice(&pack_string(""));

        let info = ObjectInfo::from_bytes(&data).unwrap();

        assert_eq!(info.format, ObjectFormat::Mp3);
        assert_eq!(info.size, 5_000_000);
        assert_eq!(info.filename, "song.mp3");
        assert!(!info.is_folder());
        assert!(info.is_file());
    }

    #[test]
    fn parse_folder_object_info() {
        let mut data = Vec::new();
        data.extend_from_slice(&pack_u32(0x00010001));
        data.extend_from_slice(&pack_u16(0x3001));        // ObjectFormat = Association
        data.extend_from_slice(&pack_u16(0x0000));
        data.extend_from_slice(&pack_u32(0));             // Size = 0 for folders
        // ... zeros for image fields
        for _ in 0..9 {
            data.extend_from_slice(&pack_u32(0));
        }
        data.extend_from_slice(&pack_u16(0x0001));        // AssociationType = Folder
        data.extend_from_slice(&pack_u32(0));
        data.extend_from_slice(&pack_u32(0));
        data.extend_from_slice(&pack_string("DCIM"));
        data.extend_from_slice(&pack_string(""));
        data.extend_from_slice(&pack_string(""));
        data.extend_from_slice(&pack_string(""));

        let info = ObjectInfo::from_bytes(&data).unwrap();

        assert_eq!(info.format, ObjectFormat::Association);
        assert!(info.is_folder());
        assert!(!info.is_file());
        assert_eq!(info.filename, "DCIM");
    }

    #[test]
    fn serialize_object_info_for_send() {
        let info = ObjectInfo {
            storage_id: StorageId(0),
            format: ObjectFormat::Text,
            protection_status: ProtectionStatus::None,
            size: 100,
            filename: "test.txt".to_string(),
            parent: ObjectHandle::ROOT,
            association_type: AssociationType::None,
            ..Default::default()
        };

        let bytes = info.to_bytes();

        // Verify we can parse it back
        let parsed = ObjectInfo::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.filename, "test.txt");
        assert_eq!(parsed.size, 100);
    }
}
```

---

## Phase 3-4: Protocol Tests (Mock Transport)

```rust
mod protocol_tests {
    use crate::transport::MockTransport;

    #[tokio::test]
    async fn test_open_session() {
        let mut mock = MockTransport::new();

        // Expect OpenSession command
        mock.expect_send(CommandContainer {
            code: OperationCode::OpenSession,
            transaction_id: 1,
            params: vec![1],
        }.to_bytes());

        // Queue OK response
        mock.queue_response(ResponseContainer {
            code: ResponseCode::Ok,
            transaction_id: 1,
            params: vec![],
        }.to_bytes());

        let session = PtpSession::open(Arc::new(mock), 1).await.unwrap();

        assert_eq!(session.session_id(), SessionId(1));
    }

    #[tokio::test]
    async fn test_get_storage_ids() {
        let mut mock = MockTransport::new();

        // Queue data response
        mock.queue_response(DataContainer {
            code: OperationCode::GetStorageIds,
            transaction_id: 2,
            payload: pack_u32_array(&[0x00010001, 0x00010002]),
        }.to_bytes());

        // Queue OK response
        mock.queue_response(ResponseContainer {
            code: ResponseCode::Ok,
            transaction_id: 2,
            params: vec![],
        }.to_bytes());

        let session = /* ... */;
        let storage_ids = session.get_storage_ids().await.unwrap();

        assert_eq!(storage_ids, vec![StorageId(0x00010001), StorageId(0x00010002)]);
    }

    #[tokio::test]
    async fn test_protocol_error_handling() {
        let mut mock = MockTransport::new();

        mock.queue_response(ResponseContainer {
            code: ResponseCode::InvalidObjectHandle,
            transaction_id: 5,
            params: vec![],
        }.to_bytes());

        let session = /* ... */;
        let result = session.get_object_info(ObjectHandle(999)).await;

        assert!(matches!(
            result,
            Err(Error::Protocol {
                code: ResponseCode::InvalidObjectHandle,
                operation: OperationCode::GetObjectInfo,
            })
        ));
    }

    #[tokio::test]
    async fn test_transaction_id_increment() {
        let mut mock = MockTransport::new();

        // First operation
        mock.expect_send(/* transaction_id = 1 */);
        mock.queue_response(/* OK */);

        // Second operation
        mock.expect_send(/* transaction_id = 2 */);
        mock.queue_response(/* OK */);

        let session = /* ... */;
        session.get_storage_ids().await.unwrap();
        session.get_storage_ids().await.unwrap();

        // Verify transaction IDs incremented
        let sends = mock.get_sends();
        // Extract and verify transaction IDs
    }
}
```

---

## Phase 6: Integration Tests

```rust
// tests/integration.rs

/// These tests require a real MTP device connected
/// Run with: cargo test --test integration -- --ignored

#[tokio::test]
#[ignore]
async fn test_device_connection() {
    let device = MtpDevice::open_first().await
        .expect("No MTP device found. Connect an Android phone in MTP mode.");

    let info = device.device_info();
    println!("Connected to: {} {}", info.manufacturer, info.model);

    assert!(!info.manufacturer.is_empty());
    assert!(!info.model.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_list_storages() {
    let device = MtpDevice::open_first().await.unwrap();
    let storages = device.storages().await.unwrap();

    assert!(!storages.is_empty(), "Device should have at least one storage");

    for storage in &storages {
        let info = storage.info();
        println!("Storage: {} ({} / {} bytes)",
                 info.description,
                 info.free_space_bytes,
                 info.max_capacity);
    }
}

#[tokio::test]
#[ignore]
async fn test_list_root_folder() {
    let device = MtpDevice::open_first().await.unwrap();
    let storage = &device.storages().await.unwrap()[0];
    let objects = storage.list_objects(None).await.unwrap();

    // Most Android devices have these folders
    let names: Vec<_> = objects.iter().map(|o| &o.filename).collect();
    println!("Root folder contents: {:?}", names);

    // Should have at least some folders
    assert!(objects.iter().any(|o| o.is_folder()));
}

#[tokio::test]
#[ignore]
async fn test_upload_download_delete() {
    let device = MtpDevice::open_first().await.unwrap();
    let storage = &device.storages().await.unwrap()[0];

    // Create test content
    let content = format!("Test file created at {:?}", std::time::SystemTime::now());
    let content_bytes = content.as_bytes();

    // Upload
    let info = NewObjectInfo::file("mtp-rs-test.txt", content_bytes.len() as u64);
    let handle = storage.upload(
        None,
        info,
        futures::stream::once(async { Ok(Bytes::from(content_bytes.to_vec())) }),
    ).await.expect("Upload failed");

    println!("Uploaded file with handle: {:?}", handle);

    // Download
    let downloaded = storage.download(handle)
        .collect()
        .await
        .expect("Download failed");

    assert_eq!(downloaded, content_bytes, "Downloaded content doesn't match");

    // Delete
    storage.delete(handle).await.expect("Delete failed");

    // Verify deleted
    let result = storage.get_object_info(handle).await;
    assert!(matches!(result, Err(Error::Protocol {
        code: ResponseCode::InvalidObjectHandle,
        ..
    })));
}

#[tokio::test]
#[ignore]
async fn test_create_and_delete_folder() {
    let device = MtpDevice::open_first().await.unwrap();
    let storage = &device.storages().await.unwrap()[0];

    // Create folder
    let handle = storage.create_folder(None, "mtp-rs-test-folder")
        .await
        .expect("Create folder failed");

    // Verify it exists
    let info = storage.get_object_info(handle).await.unwrap();
    assert!(info.is_folder());
    assert_eq!(info.filename, "mtp-rs-test-folder");

    // Delete it
    storage.delete(handle).await.expect("Delete folder failed");
}

#[tokio::test]
#[ignore]
async fn test_large_file_streaming() {
    let device = MtpDevice::open_first().await.unwrap();
    let storage = &device.storages().await.unwrap()[0];

    // Find a large file (e.g., in DCIM)
    let objects = storage.list_objects_recursive(None).await.unwrap();
    let large_file = objects.iter()
        .filter(|o| o.is_file() && o.size > 1_000_000)
        .next()
        .expect("No large file found for testing");

    println!("Downloading {} ({} bytes)", large_file.filename, large_file.size);

    let mut downloaded = 0u64;
    let mut stream = storage.download(large_file.handle);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.expect("Download error");
        downloaded += chunk.data.len() as u64;

        if let Some(total) = chunk.total_bytes {
            println!("Progress: {:.1}%", downloaded as f64 / total as f64 * 100.0);
        }
    }

    assert_eq!(downloaded, large_file.size);
}
```

---

## Running Tests

```bash
# Unit tests (no device needed)
cargo test

# With property-based tests
cargo test --features proptest

# Integration tests (requires device)
cargo test --test integration -- --ignored

# Specific test
cargo test test_pack_string

# With output
cargo test -- --nocapture
```
