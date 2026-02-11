#![allow(unsafe_op_in_unsafe_fn)]

//! High-performance Base64 encoding/decoding implementation
//!
//! This module provides an optimized Base64 encoder/decoder using custom character set:
//! - Character set: `-AaBbCcDdEeFfGgHhIiJjKkLlMmNnOoPpQqRrSsTtUuVvWwXxYyZz1032547698_`
//! - Features: URL safe, no padding characters needed

/// Base64 character set
const BASE64_CHARS: &[u8; 64] = b"-AaBbCcDdEeFfGgHhIiJjKkLlMmNnOoPpQqRrSsTtUuVvWwXxYyZz1032547698_";

/// Base64 decode lookup table
const BASE64_DECODE_TABLE: [u8; 256] = {
    let mut table = [0xFF_u8; 256];
    let mut i = 0;
    while i < BASE64_CHARS.len() {
        table[BASE64_CHARS[i] as usize] = i as u8;
        i += 1;
    }
    table
};

/// Calculate exact length after encoding
#[inline]
pub const fn encoded_len(input_len: usize) -> usize {
    let d = input_len / 3;
    let r = input_len % 3;

    (if r > 0 { d + 1 } else { d }) * 4
        - match r {
            1 => 2, // 1 byte encodes to 2 characters
            2 => 1, // 2 bytes encode to 3 characters
            0 => 0, // 3 bytes encode to 4 characters
            _ => unreachable!(),
        }
}

/// Calculate exact length after decoding
#[inline]
pub const fn decoded_len(encoded_len: usize) -> Option<usize> {
    match encoded_len % 4 {
        0 => Some((encoded_len / 4) * 3),
        2 => Some((encoded_len / 4) * 3 + 1),
        3 => Some((encoded_len / 4) * 3 + 2),
        1 => None, // Invalid length (% 4 == 1)
        _ => unreachable!(),
    }
}

/// Encode byte data to provided buffer
///
/// # Safety
///
/// Caller must ensure:
/// - input.len() bytes are readable
/// - output has encoded_len(input.len()) bytes writable
#[inline]
pub unsafe fn encode_to_slice_unchecked(input: &[u8], output: &mut [u8]) {
    let chunks_exact = input.chunks_exact(3);
    let remainder = chunks_exact.remainder();
    let mut j = 0;

    // Main loop: use chunks_exact for better compiler optimization
    for chunk in chunks_exact {
        let b1 = *chunk.get_unchecked(0);
        let b2 = *chunk.get_unchecked(1);
        let b3 = *chunk.get_unchecked(2);

        let n = ((b1 as u32) << 16) | ((b2 as u32) << 8) | (b3 as u32);

        *output.get_unchecked_mut(j) = BASE64_CHARS[(n >> 18) as usize];
        *output.get_unchecked_mut(j + 1) = BASE64_CHARS[((n >> 12) & 0x3F) as usize];
        *output.get_unchecked_mut(j + 2) = BASE64_CHARS[((n >> 6) & 0x3F) as usize];
        *output.get_unchecked_mut(j + 3) = BASE64_CHARS[(n & 0x3F) as usize];

        j += 4;
    }

    // Handle remaining bytes
    match remainder.len() {
        1 => {
            let b1 = *remainder.get_unchecked(0);
            let n = (b1 as u32) << 16;

            *output.get_unchecked_mut(j) = BASE64_CHARS[(n >> 18) as usize];
            *output.get_unchecked_mut(j + 1) = BASE64_CHARS[((n >> 12) & 0x3F) as usize];
        }
        2 => {
            let b1 = *remainder.get_unchecked(0);
            let b2 = *remainder.get_unchecked(1);
            let n = ((b1 as u32) << 16) | ((b2 as u32) << 8);

            *output.get_unchecked_mut(j) = BASE64_CHARS[(n >> 18) as usize];
            *output.get_unchecked_mut(j + 1) = BASE64_CHARS[((n >> 12) & 0x3F) as usize];
            *output.get_unchecked_mut(j + 2) = BASE64_CHARS[((n >> 6) & 0x3F) as usize];
        }
        0 => {}
        _ => ::core::hint::unreachable_unchecked(),
    }
}

/// Decode Base64 data to provided buffer
///
/// # Safety
///
/// Caller must ensure:
/// - input is valid base64 data (all characters in character set, length % 4 != 1)
/// - output has decoded_len(input.len()) bytes writable
#[inline]
pub unsafe fn decode_to_slice_unchecked(input: &[u8], output: &mut [u8]) {
    let chunks = input.chunks_exact(4);
    let remainder = chunks.remainder();
    let mut j = 0;

    // Main loop: use chunks_exact for optimization
    for chunk in chunks {
        let c1 = BASE64_DECODE_TABLE[*chunk.get_unchecked(0) as usize];
        let c2 = BASE64_DECODE_TABLE[*chunk.get_unchecked(1) as usize];
        let c3 = BASE64_DECODE_TABLE[*chunk.get_unchecked(2) as usize];
        let c4 = BASE64_DECODE_TABLE[*chunk.get_unchecked(3) as usize];

        let n = ((c1 as u32) << 18) | ((c2 as u32) << 12) | ((c3 as u32) << 6) | (c4 as u32);

        *output.get_unchecked_mut(j) = (n >> 16) as u8;
        *output.get_unchecked_mut(j + 1) = (n >> 8) as u8;
        *output.get_unchecked_mut(j + 2) = n as u8;

        j += 3;
    }

    // Handle remaining 2 or 3 characters
    match remainder.len() {
        2 => {
            let c1 = BASE64_DECODE_TABLE[*remainder.get_unchecked(0) as usize];
            let c2 = BASE64_DECODE_TABLE[*remainder.get_unchecked(1) as usize];

            *output.get_unchecked_mut(j) = (c1 << 2) | (c2 >> 4);
        }
        3 => {
            let c1 = BASE64_DECODE_TABLE[*remainder.get_unchecked(0) as usize];
            let c2 = BASE64_DECODE_TABLE[*remainder.get_unchecked(1) as usize];
            let c3 = BASE64_DECODE_TABLE[*remainder.get_unchecked(2) as usize];

            *output.get_unchecked_mut(j) = (c1 << 2) | (c2 >> 4);
            *output.get_unchecked_mut(j + 1) = (c2 << 4) | (c3 >> 2);
        }
        0 => {}
        1 => ::core::hint::unreachable_unchecked(),
        _ => ::core::hint::unreachable_unchecked(),
    }
}

/// Encode to newly allocated String
#[inline]
pub fn to_base64(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    let output_len = encoded_len(bytes.len());
    let mut output: Vec<u8> = Vec::with_capacity(output_len);

    unsafe {
        encode_to_slice_unchecked(
            bytes,
            core::slice::from_raw_parts_mut(output.as_mut_ptr(), output_len),
        );
        output.set_len(output_len);
        String::from_utf8_unchecked(output)
    }
}

/// Decode to newly allocated Vec
#[inline]
pub fn from_base64(input: &str) -> Option<Vec<u8>> {
    let input = input.as_bytes();
    let len = input.len();

    // Length check
    if len == 0 {
        return Some(Vec::new());
    }

    let output_len = decoded_len(len)?;

    // Character check - use iterator method
    if input.iter().any(|&b| BASE64_DECODE_TABLE[b as usize] == 0xFF) {
        return None;
    }

    let mut output: Vec<u8> = Vec::with_capacity(output_len);

    unsafe {
        decode_to_slice_unchecked(
            input,
            core::slice::from_raw_parts_mut(output.as_mut_ptr(), output_len),
        );
        output.set_len(output_len);
        Some(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        assert_eq!(to_base64(b""), "");
        assert_eq!(from_base64("").unwrap(), b"");
    }

    #[test]
    fn test_basic() {
        let test_cases = [
            (b"f" as &[u8], "Zg"),
            (b"fo", "Zm8"),
            (b"foo", "Zm8v"),
            (b"foob", "Zm8vYg"),
            (b"fooba", "Zm8vYmE"),
            (b"foobar", "Zm8vYmFy"),
        ];

        for (input, expected) in test_cases {
            let encoded = to_base64(input);
            assert_eq!(encoded, expected);
            assert_eq!(from_base64(&encoded).unwrap(), input);
        }
    }

    #[test]
    fn test_length_calculation() {
        assert_eq!(encoded_len(0), 0);
        assert_eq!(encoded_len(1), 2);
        assert_eq!(encoded_len(2), 3);
        assert_eq!(encoded_len(3), 4);
        assert_eq!(encoded_len(4), 6);
        assert_eq!(encoded_len(5), 7);
        assert_eq!(encoded_len(6), 8);

        assert_eq!(decoded_len(0), Some(0));
        assert_eq!(decoded_len(2), Some(1));
        assert_eq!(decoded_len(3), Some(2));
        assert_eq!(decoded_len(4), Some(3));
        assert_eq!(decoded_len(6), Some(4));
        assert_eq!(decoded_len(7), Some(5));
        assert_eq!(decoded_len(8), Some(6));
    }

    #[test]
    fn test_invalid_input() {
        assert!(from_base64("!@#$").is_none());
        assert!(from_base64("ABC").is_none()); // length % 4 == 1
    }
}
