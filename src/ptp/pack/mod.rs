//! Binary serialization/deserialization primitives for MTP/PTP.
//!
//! All multi-byte values are little-endian.

mod datetime;

pub use datetime::{pack_datetime, unpack_datetime, DateTime};

// --- Primitive packing functions ---

/// Pack a u8 value into a 1-byte array.
#[inline]
pub fn pack_u8(val: u8) -> [u8; 1] {
    [val]
}

/// Pack a u16 value into a 2-byte array (little-endian).
#[inline]
pub fn pack_u16(val: u16) -> [u8; 2] {
    val.to_le_bytes()
}

/// Pack a u32 value into a 4-byte array (little-endian).
#[inline]
pub fn pack_u32(val: u32) -> [u8; 4] {
    val.to_le_bytes()
}

/// Pack a u64 value into an 8-byte array (little-endian).
#[inline]
pub fn pack_u64(val: u64) -> [u8; 8] {
    val.to_le_bytes()
}

/// Pack a signed 8-bit integer.
#[inline]
pub fn pack_i8(val: i8) -> [u8; 1] {
    [val as u8]
}

/// Pack a signed 16-bit integer (little-endian).
#[inline]
pub fn pack_i16(val: i16) -> [u8; 2] {
    val.to_le_bytes()
}

/// Pack a signed 32-bit integer (little-endian).
#[inline]
pub fn pack_i32(val: i32) -> [u8; 4] {
    val.to_le_bytes()
}

/// Pack a signed 64-bit integer (little-endian).
#[inline]
pub fn pack_i64(val: i64) -> [u8; 8] {
    val.to_le_bytes()
}

// --- Primitive unpacking functions ---

/// Unpack a u8 value from a buffer.
pub fn unpack_u8(buf: &[u8]) -> Result<u8, crate::Error> {
    if buf.is_empty() {
        return Err(crate::Error::invalid_data(
            "insufficient bytes for u8: need 1, have 0",
        ));
    }
    Ok(buf[0])
}

/// Unpack a u16 value from a buffer (little-endian).
pub fn unpack_u16(buf: &[u8]) -> Result<u16, crate::Error> {
    if buf.len() < 2 {
        return Err(crate::Error::invalid_data(format!(
            "insufficient bytes for u16: need 2, have {}",
            buf.len()
        )));
    }
    Ok(u16::from_le_bytes([buf[0], buf[1]]))
}

/// Unpack a u32 value from a buffer (little-endian).
pub fn unpack_u32(buf: &[u8]) -> Result<u32, crate::Error> {
    if buf.len() < 4 {
        return Err(crate::Error::invalid_data(format!(
            "insufficient bytes for u32: need 4, have {}",
            buf.len()
        )));
    }
    Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]))
}

/// Unpack a u64 value from a buffer (little-endian).
pub fn unpack_u64(buf: &[u8]) -> Result<u64, crate::Error> {
    if buf.len() < 8 {
        return Err(crate::Error::invalid_data(format!(
            "insufficient bytes for u64: need 8, have {}",
            buf.len()
        )));
    }
    Ok(u64::from_le_bytes([
        buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
    ]))
}

/// Unpack a signed 8-bit integer from a buffer.
pub fn unpack_i8(buf: &[u8]) -> Result<i8, crate::Error> {
    if buf.is_empty() {
        return Err(crate::Error::invalid_data(
            "insufficient bytes for i8: need 1, have 0",
        ));
    }
    Ok(buf[0] as i8)
}

/// Unpack a signed 16-bit integer from a buffer (little-endian).
pub fn unpack_i16(buf: &[u8]) -> Result<i16, crate::Error> {
    if buf.len() < 2 {
        return Err(crate::Error::invalid_data(format!(
            "insufficient bytes for i16: need 2, have {}",
            buf.len()
        )));
    }
    Ok(i16::from_le_bytes([buf[0], buf[1]]))
}

/// Unpack a signed 32-bit integer from a buffer (little-endian).
pub fn unpack_i32(buf: &[u8]) -> Result<i32, crate::Error> {
    if buf.len() < 4 {
        return Err(crate::Error::invalid_data(format!(
            "insufficient bytes for i32: need 4, have {}",
            buf.len()
        )));
    }
    Ok(i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]))
}

/// Unpack a signed 64-bit integer from a buffer (little-endian).
pub fn unpack_i64(buf: &[u8]) -> Result<i64, crate::Error> {
    if buf.len() < 8 {
        return Err(crate::Error::invalid_data(format!(
            "insufficient bytes for i64: need 8, have {}",
            buf.len()
        )));
    }
    Ok(i64::from_le_bytes([
        buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
    ]))
}

// --- String encoding/decoding ---

/// Pack a string into MTP format (UTF-16LE with length prefix).
///
/// MTP strings are encoded as:
/// 1. 1 byte: Number of characters (including null terminator)
/// 2. N * 2 bytes: UTF-16LE encoded characters
/// 3. 2 bytes: Null terminator (0x0000)
///
/// Empty string: Single byte 0x00
pub fn pack_string(s: &str) -> Vec<u8> {
    if s.is_empty() {
        return vec![0x00];
    }

    // Encode to UTF-16
    let utf16: Vec<u16> = s.encode_utf16().collect();

    // Length includes the null terminator
    let len = utf16.len() + 1;

    // Allocate result: 1 byte length + (len * 2) bytes for UTF-16 data
    let mut result = Vec::with_capacity(1 + len * 2);

    // Length byte (number of characters including null terminator)
    result.push(len as u8);

    // UTF-16LE encoded characters
    for code_unit in &utf16 {
        result.extend_from_slice(&code_unit.to_le_bytes());
    }

    // Null terminator
    result.extend_from_slice(&[0x00, 0x00]);

    result
}

/// Unpack an MTP string from a buffer.
///
/// Returns the decoded string and the number of bytes consumed.
pub fn unpack_string(buf: &[u8]) -> Result<(String, usize), crate::Error> {
    if buf.is_empty() {
        return Err(crate::Error::invalid_data(
            "insufficient bytes for string length",
        ));
    }

    let len = buf[0] as usize;

    // Empty string
    if len == 0 {
        return Ok((String::new(), 1));
    }

    // Calculate required bytes: 1 (length) + len * 2 (UTF-16 code units)
    let required = 1 + len * 2;
    if buf.len() < required {
        return Err(crate::Error::invalid_data(format!(
            "insufficient bytes for string: need {}, have {}",
            required,
            buf.len()
        )));
    }

    // Decode UTF-16LE code units
    let mut code_units = Vec::with_capacity(len);
    for i in 0..len {
        let offset = 1 + i * 2;
        let code_unit = u16::from_le_bytes([buf[offset], buf[offset + 1]]);
        code_units.push(code_unit);
    }

    // Remove null terminator if present
    if code_units.last() == Some(&0) {
        code_units.pop();
    }

    // Decode UTF-16 to String
    let s = String::from_utf16(&code_units)
        .map_err(|_| crate::Error::invalid_data("invalid UTF-16 encoding"))?;

    Ok((s, required))
}

// --- Array encoding/decoding ---

/// Pack a u16 array into MTP format.
///
/// Arrays are encoded as:
/// 1. 4 bytes: Element count (u32, little-endian)
/// 2. N * 2 bytes: Elements (u16, little-endian each)
pub fn pack_u16_array(arr: &[u16]) -> Vec<u8> {
    let mut result = Vec::with_capacity(4 + arr.len() * 2);

    // Element count
    result.extend_from_slice(&pack_u32(arr.len() as u32));

    // Elements
    for &val in arr {
        result.extend_from_slice(&pack_u16(val));
    }

    result
}

/// Pack a u32 array into MTP format.
///
/// Arrays are encoded as:
/// 1. 4 bytes: Element count (u32, little-endian)
/// 2. N * 4 bytes: Elements (u32, little-endian each)
pub fn pack_u32_array(arr: &[u32]) -> Vec<u8> {
    let mut result = Vec::with_capacity(4 + arr.len() * 4);

    // Element count
    result.extend_from_slice(&pack_u32(arr.len() as u32));

    // Elements
    for &val in arr {
        result.extend_from_slice(&pack_u32(val));
    }

    result
}

/// Unpack a u16 array from a buffer.
///
/// Returns the array and the number of bytes consumed.
pub fn unpack_u16_array(buf: &[u8]) -> Result<(Vec<u16>, usize), crate::Error> {
    if buf.len() < 4 {
        return Err(crate::Error::invalid_data(format!(
            "insufficient bytes for array count: need 4, have {}",
            buf.len()
        )));
    }

    let count = unpack_u32(buf)? as usize;
    let required = 4 + count * 2;

    if buf.len() < required {
        return Err(crate::Error::invalid_data(format!(
            "insufficient bytes for u16 array: need {}, have {}",
            required,
            buf.len()
        )));
    }

    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let offset = 4 + i * 2;
        result.push(unpack_u16(&buf[offset..])?);
    }

    Ok((result, required))
}

/// Unpack a u32 array from a buffer.
///
/// Returns the array and the number of bytes consumed.
pub fn unpack_u32_array(buf: &[u8]) -> Result<(Vec<u32>, usize), crate::Error> {
    if buf.len() < 4 {
        return Err(crate::Error::invalid_data(format!(
            "insufficient bytes for array count: need 4, have {}",
            buf.len()
        )));
    }

    let count = unpack_u32(buf)? as usize;
    let required = 4 + count * 4;

    if buf.len() < required {
        return Err(crate::Error::invalid_data(format!(
            "insufficient bytes for u32 array: need {}, have {}",
            required,
            buf.len()
        )));
    }

    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let offset = 4 + i * 4;
        result.push(unpack_u32(&buf[offset..])?);
    }

    Ok((result, required))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Primitive packing tests ---

    #[test]
    fn pack_u8_test() {
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
        assert_eq!(
            pack_u64(0x0102030405060708),
            [0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
        );
    }

    // --- Primitive unpacking tests ---

    #[test]
    fn unpack_u8_test() {
        assert_eq!(unpack_u8(&[0x00]).unwrap(), 0x00);
        assert_eq!(unpack_u8(&[0xFF]).unwrap(), 0xFF);
        assert_eq!(unpack_u8(&[0x42]).unwrap(), 0x42);
        assert_eq!(unpack_u8(&[0x42, 0x00]).unwrap(), 0x42); // Extra bytes ignored
    }

    #[test]
    fn unpack_u16_little_endian() {
        assert_eq!(unpack_u16(&[0x00, 0x00]).unwrap(), 0x0000);
        assert_eq!(unpack_u16(&[0xFF, 0xFF]).unwrap(), 0xFFFF);
        assert_eq!(unpack_u16(&[0x34, 0x12]).unwrap(), 0x1234);
        assert_eq!(unpack_u16(&[0x01, 0x00]).unwrap(), 0x0001);
    }

    #[test]
    fn unpack_u32_little_endian() {
        assert_eq!(unpack_u32(&[0x00, 0x00, 0x00, 0x00]).unwrap(), 0x00000000);
        assert_eq!(unpack_u32(&[0xFF, 0xFF, 0xFF, 0xFF]).unwrap(), 0xFFFFFFFF);
        assert_eq!(unpack_u32(&[0x78, 0x56, 0x34, 0x12]).unwrap(), 0x12345678);
        assert_eq!(unpack_u32(&[0x01, 0x00, 0x00, 0x00]).unwrap(), 0x00000001);
    }

    #[test]
    fn unpack_u64_little_endian() {
        assert_eq!(
            unpack_u64(&[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]).unwrap(),
            0x0102030405060708
        );
    }

    // --- Primitive unpacking error tests ---

    #[test]
    fn unpack_u8_insufficient_bytes() {
        assert!(unpack_u8(&[]).is_err());
    }

    #[test]
    fn unpack_u16_insufficient_bytes() {
        assert!(unpack_u16(&[]).is_err());
        assert!(unpack_u16(&[0x00]).is_err());
    }

    #[test]
    fn unpack_u32_insufficient_bytes() {
        assert!(unpack_u32(&[]).is_err());
        assert!(unpack_u32(&[0x00]).is_err());
        assert!(unpack_u32(&[0x00, 0x00]).is_err());
        assert!(unpack_u32(&[0x00, 0x00, 0x00]).is_err());
    }

    #[test]
    fn unpack_u64_insufficient_bytes() {
        assert!(unpack_u64(&[]).is_err());
        assert!(unpack_u64(&[0x00; 7]).is_err());
    }

    // --- Primitive round-trip tests ---

    #[test]
    fn roundtrip_u8() {
        for val in [0u8, 1, 127, 128, 255] {
            assert_eq!(unpack_u8(&pack_u8(val)).unwrap(), val);
        }
    }

    #[test]
    fn roundtrip_u16() {
        for val in [0u16, 1, 255, 256, 0x1234, 0xFFFF] {
            assert_eq!(unpack_u16(&pack_u16(val)).unwrap(), val);
        }
    }

    #[test]
    fn roundtrip_u32() {
        for val in [0u32, 1, 255, 256, 0x12345678, 0xFFFFFFFF] {
            assert_eq!(unpack_u32(&pack_u32(val)).unwrap(), val);
        }
    }

    #[test]
    fn roundtrip_u64() {
        for val in [0u64, 1, 255, 256, 0x0102030405060708, 0xFFFFFFFFFFFFFFFF] {
            assert_eq!(unpack_u64(&pack_u64(val)).unwrap(), val);
        }
    }

    // --- Signed integer packing tests ---

    #[test]
    fn pack_i8_test() {
        assert_eq!(pack_i8(0), [0x00]);
        assert_eq!(pack_i8(1), [0x01]);
        assert_eq!(pack_i8(-1), [0xFF]);
        assert_eq!(pack_i8(127), [0x7F]);
        assert_eq!(pack_i8(-128), [0x80]);
    }

    #[test]
    fn pack_i16_little_endian() {
        assert_eq!(pack_i16(0), [0x00, 0x00]);
        assert_eq!(pack_i16(1), [0x01, 0x00]);
        assert_eq!(pack_i16(-1), [0xFF, 0xFF]);
        assert_eq!(pack_i16(0x1234), [0x34, 0x12]);
        assert_eq!(pack_i16(-2), [0xFE, 0xFF]);
        assert_eq!(pack_i16(32767), [0xFF, 0x7F]);
        assert_eq!(pack_i16(-32768), [0x00, 0x80]);
    }

    #[test]
    fn pack_i32_little_endian() {
        assert_eq!(pack_i32(0), [0x00, 0x00, 0x00, 0x00]);
        assert_eq!(pack_i32(1), [0x01, 0x00, 0x00, 0x00]);
        assert_eq!(pack_i32(-1), [0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(pack_i32(0x12345678), [0x78, 0x56, 0x34, 0x12]);
        assert_eq!(pack_i32(-2), [0xFE, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn pack_i64_little_endian() {
        assert_eq!(
            pack_i64(0x0102030405060708),
            [0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
        );
        assert_eq!(
            pack_i64(-1),
            [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
        );
    }

    // --- Signed integer unpacking tests ---

    #[test]
    fn unpack_i8_test() {
        assert_eq!(unpack_i8(&[0x00]).unwrap(), 0);
        assert_eq!(unpack_i8(&[0x01]).unwrap(), 1);
        assert_eq!(unpack_i8(&[0xFF]).unwrap(), -1);
        assert_eq!(unpack_i8(&[0x7F]).unwrap(), 127);
        assert_eq!(unpack_i8(&[0x80]).unwrap(), -128);
        assert_eq!(unpack_i8(&[0x80, 0x00]).unwrap(), -128); // Extra bytes ignored
    }

    #[test]
    fn unpack_i16_little_endian() {
        assert_eq!(unpack_i16(&[0x00, 0x00]).unwrap(), 0);
        assert_eq!(unpack_i16(&[0xFF, 0xFF]).unwrap(), -1);
        assert_eq!(unpack_i16(&[0x34, 0x12]).unwrap(), 0x1234);
        assert_eq!(unpack_i16(&[0xFE, 0xFF]).unwrap(), -2);
        assert_eq!(unpack_i16(&[0xFF, 0x7F]).unwrap(), 32767);
        assert_eq!(unpack_i16(&[0x00, 0x80]).unwrap(), -32768);
    }

    #[test]
    fn unpack_i32_little_endian() {
        assert_eq!(unpack_i32(&[0x00, 0x00, 0x00, 0x00]).unwrap(), 0);
        assert_eq!(unpack_i32(&[0xFF, 0xFF, 0xFF, 0xFF]).unwrap(), -1);
        assert_eq!(unpack_i32(&[0x78, 0x56, 0x34, 0x12]).unwrap(), 0x12345678);
        assert_eq!(unpack_i32(&[0xFE, 0xFF, 0xFF, 0xFF]).unwrap(), -2);
    }

    #[test]
    fn unpack_i64_little_endian() {
        assert_eq!(
            unpack_i64(&[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]).unwrap(),
            0x0102030405060708
        );
        assert_eq!(
            unpack_i64(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]).unwrap(),
            -1
        );
    }

    // --- Signed integer unpacking error tests ---

    #[test]
    fn unpack_i8_insufficient_bytes() {
        assert!(unpack_i8(&[]).is_err());
    }

    #[test]
    fn unpack_i16_insufficient_bytes() {
        assert!(unpack_i16(&[]).is_err());
        assert!(unpack_i16(&[0x00]).is_err());
    }

    #[test]
    fn unpack_i32_insufficient_bytes() {
        assert!(unpack_i32(&[]).is_err());
        assert!(unpack_i32(&[0x00]).is_err());
        assert!(unpack_i32(&[0x00, 0x00]).is_err());
        assert!(unpack_i32(&[0x00, 0x00, 0x00]).is_err());
    }

    #[test]
    fn unpack_i64_insufficient_bytes() {
        assert!(unpack_i64(&[]).is_err());
        assert!(unpack_i64(&[0x00; 7]).is_err());
    }

    // --- Signed integer round-trip tests ---

    #[test]
    fn roundtrip_i8() {
        for val in [0i8, 1, -1, 127, -128, 42, -42] {
            assert_eq!(unpack_i8(&pack_i8(val)).unwrap(), val);
        }
    }

    #[test]
    fn roundtrip_i16() {
        for val in [0i16, 1, -1, 255, -255, 32767, -32768, 0x1234, -0x1234] {
            assert_eq!(unpack_i16(&pack_i16(val)).unwrap(), val);
        }
    }

    #[test]
    fn roundtrip_i32() {
        for val in [0i32, 1, -1, 255, -255, 0x12345678, -0x12345678] {
            assert_eq!(unpack_i32(&pack_i32(val)).unwrap(), val);
        }
    }

    #[test]
    fn roundtrip_i64() {
        for val in [
            0i64,
            1,
            -1,
            255,
            -255,
            0x0102030405060708,
            -0x0102030405060708,
        ] {
            assert_eq!(unpack_i64(&pack_i64(val)).unwrap(), val);
        }
    }

    // --- String packing tests ---

    #[test]
    fn pack_string_empty() {
        assert_eq!(pack_string(""), vec![0x00]);
    }

    #[test]
    fn pack_string_ascii() {
        assert_eq!(
            pack_string("Hi"),
            vec![
                0x03, // length = 3
                0x48, 0x00, // 'H'
                0x69, 0x00, // 'i'
                0x00, 0x00, // null
            ]
        );
    }

    #[test]
    fn pack_string_single_char() {
        assert_eq!(
            pack_string("A"),
            vec![
                0x02, // length = 2 (char + null)
                0x41, 0x00, // 'A'
                0x00, 0x00, // null
            ]
        );
    }

    #[test]
    fn pack_string_japanese() {
        // Test with Japanese characters (BMP characters, no surrogate pairs needed)
        let s = "\u{3053}\u{3093}\u{306B}\u{3061}\u{306F}"; // "konnichiwa" in hiragana
        let packed = pack_string(s);
        assert_eq!(packed[0], 6); // 5 chars + null
        assert_eq!(packed.len(), 1 + 6 * 2); // 1 byte length + 6 UTF-16 code units * 2 bytes
    }

    #[test]
    fn pack_string_emoji_surrogate_pair() {
        // Emoji outside BMP require surrogate pairs in UTF-16
        let s = "\u{1F600}"; // Grinning face emoji
        let packed = pack_string(s);
        // UTF-16 encoding: surrogate pair (2 code units) + null = 3 code units
        assert_eq!(packed[0], 3);
        // High surrogate: 0xD83D, Low surrogate: 0xDE00
        assert_eq!(packed[1], 0x3D); // Low byte of 0xD83D
        assert_eq!(packed[2], 0xD8); // High byte of 0xD83D
        assert_eq!(packed[3], 0x00); // Low byte of 0xDE00
        assert_eq!(packed[4], 0xDE); // High byte of 0xDE00
        assert_eq!(packed[5], 0x00); // Null low
        assert_eq!(packed[6], 0x00); // Null high
    }

    // --- String unpacking tests ---

    #[test]
    fn unpack_string_empty() {
        let (s, consumed) = unpack_string(&[0x00]).unwrap();
        assert_eq!(s, "");
        assert_eq!(consumed, 1);
    }

    #[test]
    fn unpack_string_ascii() {
        let buf = vec![
            0x03, // length = 3
            0x48, 0x00, // 'H'
            0x69, 0x00, // 'i'
            0x00, 0x00, // null
        ];
        let (s, consumed) = unpack_string(&buf).unwrap();
        assert_eq!(s, "Hi");
        assert_eq!(consumed, 7);
    }

    #[test]
    fn unpack_string_with_extra_data() {
        let buf = vec![
            0x02, // length = 2
            0x41, 0x00, // 'A'
            0x00, 0x00, // null
            0xFF, 0xFF, // extra data (should be ignored)
        ];
        let (s, consumed) = unpack_string(&buf).unwrap();
        assert_eq!(s, "A");
        assert_eq!(consumed, 5);
    }

    #[test]
    fn unpack_string_insufficient_bytes_for_length() {
        assert!(unpack_string(&[]).is_err());
    }

    #[test]
    fn unpack_string_insufficient_bytes_for_content() {
        // Says length is 3 (6 bytes of content) but only provides 2 bytes
        assert!(unpack_string(&[0x03, 0x41, 0x00]).is_err());
    }

    #[test]
    fn unpack_string_invalid_utf16() {
        // Invalid surrogate pair: high surrogate without low surrogate
        let buf = vec![
            0x02, // length = 2
            0x00, 0xD8, // Invalid high surrogate alone (0xD800)
            0x00, 0x00, // null
        ];
        assert!(unpack_string(&buf).is_err());
    }

    // --- String round-trip tests ---

    #[test]
    fn roundtrip_string_empty() {
        let (s, _) = unpack_string(&pack_string("")).unwrap();
        assert_eq!(s, "");
    }

    #[test]
    fn roundtrip_string_ascii() {
        let original = "Hello, World!";
        let (s, _) = unpack_string(&pack_string(original)).unwrap();
        assert_eq!(s, original);
    }

    #[test]
    fn roundtrip_string_unicode() {
        let test_strings = [
            "Hello",
            "\u{00E9}",                                 // e with acute accent (BMP)
            "\u{3053}\u{3093}\u{306B}\u{3061}\u{306F}", // Japanese hiragana
            "\u{1F600}",                                // emoji (surrogate pair)
            "Mixed: Hello \u{4E16}\u{754C} \u{1F310}",  // mixed with emoji
        ];

        for original in test_strings {
            let (s, _) = unpack_string(&pack_string(original)).unwrap();
            assert_eq!(s, original, "Round-trip failed for: {}", original);
        }
    }

    // --- Array packing tests ---

    #[test]
    fn pack_u16_array_empty() {
        assert_eq!(pack_u16_array(&[]), vec![0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn pack_u16_array_single() {
        assert_eq!(
            pack_u16_array(&[0x1234]),
            vec![
                0x01, 0x00, 0x00, 0x00, // count = 1
                0x34, 0x12, // 0x1234
            ]
        );
    }

    #[test]
    fn pack_u16_array_multiple() {
        assert_eq!(
            pack_u16_array(&[1, 2, 3]),
            vec![
                0x03, 0x00, 0x00, 0x00, // count = 3
                0x01, 0x00, // 1
                0x02, 0x00, // 2
                0x03, 0x00, // 3
            ]
        );
    }

    #[test]
    fn pack_u32_array_empty() {
        assert_eq!(pack_u32_array(&[]), vec![0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn pack_u32_array_multiple() {
        assert_eq!(
            pack_u32_array(&[1, 2, 3]),
            vec![
                0x03, 0x00, 0x00, 0x00, // count = 3
                0x01, 0x00, 0x00, 0x00, // 1
                0x02, 0x00, 0x00, 0x00, // 2
                0x03, 0x00, 0x00, 0x00, // 3
            ]
        );
    }

    // --- Array unpacking tests ---

    #[test]
    fn unpack_u16_array_empty() {
        let (arr, consumed) = unpack_u16_array(&[0x00, 0x00, 0x00, 0x00]).unwrap();
        assert!(arr.is_empty());
        assert_eq!(consumed, 4);
    }

    #[test]
    fn unpack_u16_array_multiple() {
        let buf = vec![
            0x03, 0x00, 0x00, 0x00, // count = 3
            0x01, 0x00, // 1
            0x02, 0x00, // 2
            0x03, 0x00, // 3
        ];
        let (arr, consumed) = unpack_u16_array(&buf).unwrap();
        assert_eq!(arr, vec![1, 2, 3]);
        assert_eq!(consumed, 10);
    }

    #[test]
    fn unpack_u32_array_empty() {
        let (arr, consumed) = unpack_u32_array(&[0x00, 0x00, 0x00, 0x00]).unwrap();
        assert!(arr.is_empty());
        assert_eq!(consumed, 4);
    }

    #[test]
    fn unpack_u32_array_multiple() {
        let buf = vec![
            0x03, 0x00, 0x00, 0x00, // count = 3
            0x01, 0x00, 0x00, 0x00, // 1
            0x02, 0x00, 0x00, 0x00, // 2
            0x03, 0x00, 0x00, 0x00, // 3
        ];
        let (arr, consumed) = unpack_u32_array(&buf).unwrap();
        assert_eq!(arr, vec![1, 2, 3]);
        assert_eq!(consumed, 16);
    }

    #[test]
    fn unpack_u16_array_with_extra_data() {
        let buf = vec![
            0x01, 0x00, 0x00, 0x00, // count = 1
            0x34, 0x12, // 0x1234
            0xFF, 0xFF, // extra data
        ];
        let (arr, consumed) = unpack_u16_array(&buf).unwrap();
        assert_eq!(arr, vec![0x1234]);
        assert_eq!(consumed, 6);
    }

    #[test]
    fn unpack_u32_array_with_extra_data() {
        let buf = vec![
            0x01, 0x00, 0x00, 0x00, // count = 1
            0x78, 0x56, 0x34, 0x12, // 0x12345678
            0xFF, 0xFF, 0xFF, 0xFF, // extra data
        ];
        let (arr, consumed) = unpack_u32_array(&buf).unwrap();
        assert_eq!(arr, vec![0x12345678]);
        assert_eq!(consumed, 8);
    }

    // --- Array unpacking error tests ---

    #[test]
    fn unpack_u16_array_insufficient_bytes_for_count() {
        assert!(unpack_u16_array(&[]).is_err());
        assert!(unpack_u16_array(&[0x00, 0x00, 0x00]).is_err());
    }

    #[test]
    fn unpack_u16_array_insufficient_bytes_for_elements() {
        // Count says 2, but only 1 element provided
        let buf = vec![
            0x02, 0x00, 0x00, 0x00, // count = 2
            0x01, 0x00, // only 1 element
        ];
        assert!(unpack_u16_array(&buf).is_err());
    }

    #[test]
    fn unpack_u32_array_insufficient_bytes_for_count() {
        assert!(unpack_u32_array(&[]).is_err());
        assert!(unpack_u32_array(&[0x00, 0x00, 0x00]).is_err());
    }

    #[test]
    fn unpack_u32_array_insufficient_bytes_for_elements() {
        // Count says 2, but only 1 element provided
        let buf = vec![
            0x02, 0x00, 0x00, 0x00, // count = 2
            0x01, 0x00, 0x00, 0x00, // only 1 element
        ];
        assert!(unpack_u32_array(&buf).is_err());
    }

    // --- Array round-trip tests ---

    #[test]
    fn roundtrip_u16_array() {
        let test_arrays: &[&[u16]] = &[&[], &[0], &[1, 2, 3], &[0xFFFF, 0x1234, 0x0001]];

        for original in test_arrays {
            let (arr, _) = unpack_u16_array(&pack_u16_array(original)).unwrap();
            assert_eq!(&arr[..], *original);
        }
    }

    #[test]
    fn roundtrip_u32_array() {
        let test_arrays: &[&[u32]] =
            &[&[], &[0], &[1, 2, 3], &[0xFFFFFFFF, 0x12345678, 0x00000001]];

        for original in test_arrays {
            let (arr, _) = unpack_u32_array(&pack_u32_array(original)).unwrap();
            assert_eq!(&arr[..], *original);
        }
    }

    #[test]
    fn bytes_consumed_correct() {
        // Test that bytes_consumed is correctly calculated when there's extra data
        let mut buf = pack_string("test");
        buf.extend_from_slice(&[0xFF, 0xFF, 0xFF]); // Extra data

        let (_, consumed) = unpack_string(&buf).unwrap();
        assert_eq!(consumed, buf.len() - 3); // Should not include extra data
    }

    // --- Property-based tests ---

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_roundtrip_u8(val: u8) {
            let packed = pack_u8(val);
            let unpacked = unpack_u8(&packed).unwrap();
            prop_assert_eq!(unpacked, val);
        }

        #[test]
        fn prop_roundtrip_u16(val: u16) {
            let packed = pack_u16(val);
            let unpacked = unpack_u16(&packed).unwrap();
            prop_assert_eq!(unpacked, val);
        }

        #[test]
        fn prop_roundtrip_u32(val: u32) {
            let packed = pack_u32(val);
            let unpacked = unpack_u32(&packed).unwrap();
            prop_assert_eq!(unpacked, val);
        }

        #[test]
        fn prop_roundtrip_u64(val: u64) {
            let packed = pack_u64(val);
            let unpacked = unpack_u64(&packed).unwrap();
            prop_assert_eq!(unpacked, val);
        }

        #[test]
        fn prop_roundtrip_i8(val: i8) {
            let packed = pack_i8(val);
            let unpacked = unpack_i8(&packed).unwrap();
            prop_assert_eq!(unpacked, val);
        }

        #[test]
        fn prop_roundtrip_i16(val: i16) {
            let packed = pack_i16(val);
            let unpacked = unpack_i16(&packed).unwrap();
            prop_assert_eq!(unpacked, val);
        }

        #[test]
        fn prop_roundtrip_i32(val: i32) {
            let packed = pack_i32(val);
            let unpacked = unpack_i32(&packed).unwrap();
            prop_assert_eq!(unpacked, val);
        }

        #[test]
        fn prop_roundtrip_i64(val: i64) {
            let packed = pack_i64(val);
            let unpacked = unpack_i64(&packed).unwrap();
            prop_assert_eq!(unpacked, val);
        }

        #[test]
        fn prop_pack_u8_length(val: u8) {
            prop_assert_eq!(pack_u8(val).len(), 1);
        }

        #[test]
        fn prop_pack_u16_length(val: u16) {
            prop_assert_eq!(pack_u16(val).len(), 2);
        }

        #[test]
        fn prop_pack_u32_length(val: u32) {
            prop_assert_eq!(pack_u32(val).len(), 4);
        }

        #[test]
        fn prop_pack_u64_length(val: u64) {
            prop_assert_eq!(pack_u64(val).len(), 8);
        }

        #[test]
        fn prop_pack_i8_length(val: i8) {
            prop_assert_eq!(pack_i8(val).len(), 1);
        }

        #[test]
        fn prop_pack_i16_length(val: i16) {
            prop_assert_eq!(pack_i16(val).len(), 2);
        }

        #[test]
        fn prop_pack_i32_length(val: i32) {
            prop_assert_eq!(pack_i32(val).len(), 4);
        }

        #[test]
        fn prop_pack_i64_length(val: i64) {
            prop_assert_eq!(pack_i64(val).len(), 8);
        }
    }

    /// Strategy for generating valid UTF-16 compatible strings.
    /// We avoid lone surrogates which would cause UTF-16 encoding issues.
    fn valid_utf16_string() -> impl Strategy<Value = String> {
        // Generate strings from BMP characters (no surrogate issues)
        // Include ASCII, common Unicode, and some emoji (via char::from_u32)
        prop::collection::vec(
            prop::char::range('\u{0000}', '\u{D7FF}')
                .prop_union(prop::char::range('\u{E000}', '\u{FFFF}')),
            0..100,
        )
        .prop_map(|chars| chars.into_iter().collect::<String>())
    }

    proptest! {
        #[test]
        fn prop_roundtrip_string(s in valid_utf16_string()) {
            // MTP strings have max length of 255 characters (including null)
            // So limit to 254 characters to avoid overflow
            let s = if s.chars().count() > 254 {
                s.chars().take(254).collect::<String>()
            } else {
                s
            };

            let packed = pack_string(&s);
            let (unpacked, consumed) = unpack_string(&packed).unwrap();
            prop_assert_eq!(&unpacked, &s);
            prop_assert_eq!(consumed, packed.len());
        }

        #[test]
        fn prop_string_packed_length(s in valid_utf16_string()) {
            // MTP strings: 1 byte length + (chars+1) * 2 bytes (including null)
            // Empty string: just 1 byte (0x00)
            let s = if s.chars().count() > 254 {
                s.chars().take(254).collect::<String>()
            } else {
                s
            };

            let packed = pack_string(&s);

            if s.is_empty() {
                prop_assert_eq!(packed.len(), 1);
            } else {
                // UTF-16 code units (not chars, as some chars need 2 code units)
                let utf16_len: usize = s.encode_utf16().count();
                let expected_len = 1 + (utf16_len + 1) * 2; // 1 byte len + (code_units + null) * 2
                prop_assert_eq!(packed.len(), expected_len);
            }
        }

        #[test]
        fn prop_roundtrip_u16_array(arr in prop::collection::vec(any::<u16>(), 0..100)) {
            let packed = pack_u16_array(&arr);
            let (unpacked, consumed) = unpack_u16_array(&packed).unwrap();
            prop_assert_eq!(&unpacked, &arr);
            prop_assert_eq!(consumed, packed.len());
        }

        #[test]
        fn prop_roundtrip_u32_array(arr in prop::collection::vec(any::<u32>(), 0..100)) {
            let packed = pack_u32_array(&arr);
            let (unpacked, consumed) = unpack_u32_array(&packed).unwrap();
            prop_assert_eq!(&unpacked, &arr);
            prop_assert_eq!(consumed, packed.len());
        }

        #[test]
        fn prop_u16_array_packed_length(arr in prop::collection::vec(any::<u16>(), 0..100)) {
            let packed = pack_u16_array(&arr);
            // 4 bytes for count + 2 bytes per element
            let expected_len = 4 + arr.len() * 2;
            prop_assert_eq!(packed.len(), expected_len);
        }

        #[test]
        fn prop_u32_array_packed_length(arr in prop::collection::vec(any::<u32>(), 0..100)) {
            let packed = pack_u32_array(&arr);
            // 4 bytes for count + 4 bytes per element
            let expected_len = 4 + arr.len() * 4;
            prop_assert_eq!(packed.len(), expected_len);
        }

        #[test]
        fn prop_unpack_u8_ignores_extra_bytes(val: u8, extra in prop::collection::vec(any::<u8>(), 0..10)) {
            let mut buf = pack_u8(val).to_vec();
            buf.extend_from_slice(&extra);
            let unpacked = unpack_u8(&buf).unwrap();
            prop_assert_eq!(unpacked, val);
        }

        #[test]
        fn prop_unpack_u16_ignores_extra_bytes(val: u16, extra in prop::collection::vec(any::<u8>(), 0..10)) {
            let mut buf = pack_u16(val).to_vec();
            buf.extend_from_slice(&extra);
            let unpacked = unpack_u16(&buf).unwrap();
            prop_assert_eq!(unpacked, val);
        }

        #[test]
        fn prop_unpack_u32_ignores_extra_bytes(val: u32, extra in prop::collection::vec(any::<u8>(), 0..10)) {
            let mut buf = pack_u32(val).to_vec();
            buf.extend_from_slice(&extra);
            let unpacked = unpack_u32(&buf).unwrap();
            prop_assert_eq!(unpacked, val);
        }

        #[test]
        fn prop_unpack_u64_ignores_extra_bytes(val: u64, extra in prop::collection::vec(any::<u8>(), 0..10)) {
            let mut buf = pack_u64(val).to_vec();
            buf.extend_from_slice(&extra);
            let unpacked = unpack_u64(&buf).unwrap();
            prop_assert_eq!(unpacked, val);
        }
    }

    // Adversarial tests for malformed inputs

    proptest! {
        /// Truncated u16 buffer (1 byte when 2 needed) should return Err
        #[test]
        fn fuzz_unpack_u16_truncated(byte: u8) {
            let result = unpack_u16(&[byte]);
            prop_assert!(result.is_err());
        }

        /// Truncated u32 buffer (1-3 bytes when 4 needed) should return Err
        #[test]
        fn fuzz_unpack_u32_truncated(bytes in prop::collection::vec(any::<u8>(), 1..4)) {
            let result = unpack_u32(&bytes);
            prop_assert!(result.is_err());
        }

        /// Truncated u64 buffer (1-7 bytes when 8 needed) should return Err
        #[test]
        fn fuzz_unpack_u64_truncated(bytes in prop::collection::vec(any::<u8>(), 1..8)) {
            let result = unpack_u64(&bytes);
            prop_assert!(result.is_err());
        }

        /// Truncated i16 buffer should return Err
        #[test]
        fn fuzz_unpack_i16_truncated(byte: u8) {
            let result = unpack_i16(&[byte]);
            prop_assert!(result.is_err());
        }

        /// Truncated i32 buffer should return Err
        #[test]
        fn fuzz_unpack_i32_truncated(bytes in prop::collection::vec(any::<u8>(), 1..4)) {
            let result = unpack_i32(&bytes);
            prop_assert!(result.is_err());
        }

        /// Truncated i64 buffer should return Err
        #[test]
        fn fuzz_unpack_i64_truncated(bytes in prop::collection::vec(any::<u8>(), 1..8)) {
            let result = unpack_i64(&bytes);
            prop_assert!(result.is_err());
        }
        /// Random garbage bytes should either succeed or fail gracefully - NEVER panic
        #[test]
        fn fuzz_unpack_string_garbage(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
            // This should never panic, regardless of input
            let _ = unpack_string(&bytes);
        }

        /// String with claimed length larger than actual data should fail gracefully
        #[test]
        fn fuzz_unpack_string_invalid_length(
            claimed_len in 1u8..=255u8,
            actual_data in prop::collection::vec(any::<u8>(), 0..10)
        ) {
            let mut buf = vec![claimed_len];
            buf.extend_from_slice(&actual_data);
            // If claimed_len says there's more data than exists, should fail gracefully
            let result = unpack_string(&buf);
            // Either succeeds (if we got lucky with the length) or returns Err
            // but should NEVER panic
            let _ = result;
        }

        /// Empty buffer should return Err for string unpacking
        #[test]
        fn fuzz_unpack_string_empty(_dummy: u8) {
            let result = unpack_string(&[]);
            prop_assert!(result.is_err());
        }

        /// u32 array with claimed count larger than actual elements
        #[test]
        fn fuzz_unpack_u32_array_invalid_count(
            claimed_count in 2u32..1000u32,
            actual_elements in prop::collection::vec(any::<u32>(), 0..5)
        ) {
            let mut buf = pack_u32(claimed_count).to_vec();
            for elem in &actual_elements {
                buf.extend_from_slice(&pack_u32(*elem));
            }
            let result = unpack_u32_array(&buf);
            // Should fail if claimed_count > actual_elements.len()
            if claimed_count as usize > actual_elements.len() {
                prop_assert!(result.is_err());
            }
        }

        /// u16 array with claimed count larger than actual elements
        #[test]
        fn fuzz_unpack_u16_array_invalid_count(
            claimed_count in 2u32..1000u32,
            actual_elements in prop::collection::vec(any::<u16>(), 0..5)
        ) {
            let mut buf = pack_u32(claimed_count).to_vec();
            for elem in &actual_elements {
                buf.extend_from_slice(&pack_u16(*elem));
            }
            let result = unpack_u16_array(&buf);
            // Should fail if claimed_count > actual_elements.len()
            if claimed_count as usize > actual_elements.len() {
                prop_assert!(result.is_err());
            }
        }

        /// Random garbage as array should not panic
        #[test]
        fn fuzz_unpack_u32_array_garbage(bytes in prop::collection::vec(any::<u8>(), 0..50)) {
            let _ = unpack_u32_array(&bytes);
        }

        /// Random garbage as u16 array should not panic
        #[test]
        fn fuzz_unpack_u16_array_garbage(bytes in prop::collection::vec(any::<u8>(), 0..50)) {
            let _ = unpack_u16_array(&bytes);
        }

        /// Large array count that could overflow length calculations
        #[test]
        fn fuzz_u32_array_large_count(count in (u32::MAX - 100)..=u32::MAX) {
            // This could cause overflow: required = 4 + count * 4
            let buf = pack_u32(count);
            let result = unpack_u32_array(&buf);
            // Should fail gracefully (not enough data) or panic is a bug
            prop_assert!(result.is_err());
        }

        /// Large array count for u16 array
        #[test]
        fn fuzz_u16_array_large_count(count in (u32::MAX - 100)..=u32::MAX) {
            // This could cause overflow: required = 4 + count * 2
            let buf = pack_u32(count);
            let result = unpack_u16_array(&buf);
            // Should fail gracefully (not enough data) or panic is a bug
            prop_assert!(result.is_err());
        }

        /// Large string length that could overflow
        #[test]
        fn fuzz_string_length_255(
            extra_bytes in prop::collection::vec(any::<u8>(), 0..10)
        ) {
            // Length 255 requires 1 + 255 * 2 = 511 bytes
            let mut buf = vec![255u8];
            buf.extend_from_slice(&extra_bytes);
            let result = unpack_string(&buf);
            // With only a few extra bytes, this should fail
            prop_assert!(result.is_err());
        }
    }
}
