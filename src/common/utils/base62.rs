#![allow(unsafe_op_in_unsafe_fn, unused)]

//! High-performance fixed-length base62 encoder/decoder
//!
//! This module provides base62 encoding/decoding functionality optimized for `u128` type.
//!
//! # Features
//!
//! - Fixed-length output: all encoding results are exactly 22 bytes
//! - High performance: uses magic number division to avoid expensive u128 division operations
//! - Zero allocation: no heap memory allocation needed
//! - Leading zero padding: automatically pads '0' at the front for smaller values
//!
//! # Example
//!
//! ```
//! use base62_u128::{encode_fixed, decode_fixed, BASE62_LEN};
//!
//! let mut buf = [0u8; BASE62_LEN];
//! encode_fixed(12345u128, &mut buf);
//! assert_eq!(&buf[..], b"00000000000000000003D7");
//!
//! let decoded = decode_fixed(&buf).unwrap();
//! assert_eq!(decoded, 12345u128);
//! ```

use core::fmt;

// ============================================================================
// Constant definitions
// ============================================================================

/// Base62 radix
const BASE: u64 = 62;

/// Fixed length of encode output
pub const BASE62_LEN: usize = 22;

/// 62^10 - used to decompose u128 into manageable blocks
///
/// This value is carefully chosen because:
/// - It is large enough to efficiently decompose u128
/// - It is small enough to fit in u64
const BASE_TO_10: u64 = 839_299_365_868_340_224;
const BASE_TO_10_U128: u128 = BASE_TO_10 as u128;

/// Magic number for fast division - used to calculate u128 / BASE_TO_10
///
/// These constants are calculated as follows:
/// - MULTIPLY = ceil(2^(128 + SHIFT) / BASE_TO_10)
/// - SHIFT is chosen to be the minimum value that makes the result exact
const DIV_BASE_TO_10_MULTIPLY: u128 = 233_718_071_534_448_225_491_982_379_416_108_680_074;
const DIV_BASE_TO_10_SHIFT: u8 = 59;

/// Base62 character set (standard order: 0-9, A-Z, a-z)
const CHARSET: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Decode lookup table - maps ASCII characters to their base62 values
///
/// - Valid characters map to 0-61
/// - Invalid characters map to 0xFF
const DECODE_LUT: &[u8; 256] = &{
    let mut lut = [0xFF; 256];
    let mut i = 0;
    while i < 62 {
        lut[CHARSET[i] as usize] = i as u8;
        i += 1;
    }
    lut
};

// ============================================================================
// Error type
// ============================================================================

/// Base62 decode error
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DecodeError {
    /// Decode result exceeds u128 range
    ArithmeticOverflow,
    /// Encountered invalid base62 character
    InvalidCharacter {
        /// Invalid byte value
        byte: u8,
        /// Position of byte in input
        position: usize,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::ArithmeticOverflow => {
                write!(f, "decoded number would overflow u128")
            }
            DecodeError::InvalidCharacter { byte, position } => {
                write!(
                    f,
                    "invalid base62 character '{}' (0x{:02X}) at position {}",
                    byte.escape_ascii(),
                    byte,
                    position
                )
            }
        }
    }
}

impl std::error::Error for DecodeError {}

// ============================================================================
// Core algorithm
// ============================================================================

/// Use magic number to quickly calculate u128 / BASE_TO_10
///
/// # Return value
///
/// (quotient, remainder)
///
/// # Algorithm
///
/// Use fixed-point arithmetic to avoid expensive u128 division:
/// - quotient = (num * MULTIPLY) >> (128 + SHIFT)
/// - remainder = num - quotient * BASE_TO_10
#[inline(always)]
fn fast_div_base_to_10(num: u128) -> (u128, u64) {
    let quotient = mulh(num, DIV_BASE_TO_10_MULTIPLY) >> DIV_BASE_TO_10_SHIFT;
    let remainder = num - quotient * BASE_TO_10_U128;
    (quotient, remainder as u64)
}

/// Calculate high 128 bits of multiplying two u128 numbers
///
/// # Algorithm
///
/// Decompose input into 64-bit blocks for multiplication:
/// ```text
/// x = x_hi * 2^64 + x_lo
/// y = y_hi * 2^64 + y_lo
/// x * y = x_hi * y_hi * 2^128 + (x_hi * y_lo + x_lo * y_hi) * 2^64 + x_lo * y_lo
/// ```
#[inline(always)]
const fn mulh(x: u128, y: u128) -> u128 {
    let x_lo = x as u64 as u128;
    let x_hi = x >> 64;
    let y_lo = y as u64 as u128;
    let y_hi = y >> 64;

    let z0 = x_lo * y_lo;
    let z1 = x_lo * y_hi;
    let z2 = x_hi * y_lo;
    let z3 = x_hi * y_hi;

    let carry = (z0 >> 64) + (z1 as u64 as u128) + (z2 as u64 as u128);
    z3 + (z1 >> 64) + (z2 >> 64) + (carry >> 64)
}

// ============================================================================
// Public API
// ============================================================================

/// Encode u128 to fixed-length base62 string
///
/// # Parameters
///
/// - `num`: Value to encode
/// - `buf`: Output buffer, must be exactly [`BASE62_LEN`] bytes
///
/// # Performance
///
/// This function is highly optimized:
/// - Use two fast divisions to decompose u128 into three u64 blocks
/// - Each block uses native u64 operations for encoding
/// - No branch prediction failures, no memory allocation
///
/// # Example
///
/// ```
/// # use base62_u128::{encode_fixed, BASE62_LEN};
/// let mut buf = [0u8; BASE62_LEN];
/// encode_fixed(u128::MAX, &mut buf);
/// assert_eq!(&buf[..], b"7n42DGM5Tflk9n8mt7Fhc7");
/// ```
#[inline]
pub fn encode_fixed(num: u128, buf: &mut [u8; BASE62_LEN]) {
    // Decompose u128 into three blocks:
    // num = high * (62^10)^2 + mid * 62^10 + low
    let (quotient, low) = fast_div_base_to_10(num);
    let (high, mid) = fast_div_base_to_10(quotient);

    // Encode each block
    // SAFETY: All indices are known at compile time to be in range
    unsafe {
        // Low 10 bits -> buf[12..22]
        encode_u64_chunk(low, 10, buf.as_mut_ptr().add(12));
        // Mid 10 bits -> buf[2..12]
        encode_u64_chunk(mid, 10, buf.as_mut_ptr().add(2));
        // High 2 bits -> buf[0..2]
        encode_u64_chunk(high as u64, 2, buf.as_mut_ptr());
    }
}

/// Encode u64 value to base62 string of specified length
///
/// # Safety
///
/// Caller must ensure:
/// - `ptr` points to at least `len` bytes of valid memory
/// - `num` encoded will not exceed `len` characters
#[inline(always)]
unsafe fn encode_u64_chunk(mut num: u64, len: usize, ptr: *mut u8) {
    for i in (0..len).rev() {
        let digit = (num % BASE) as usize;
        num /= BASE;
        *ptr.add(i) = *CHARSET.get_unchecked(digit);
    }
}

/// Decode fixed-length base62 string to u128
///
/// # Parameters
///
/// - `buf`: Input buffer, must be exactly [`BASE62_LEN`] bytes
///
/// # Error
///
/// - [`DecodeError::InvalidCharacter`]: Input contains non-base62 characters
/// - [`DecodeError::ArithmeticOverflow`]: Decode result exceeds u128 range
///
/// # Example
///
/// ```
/// # use base62_u128::{decode_fixed, BASE62_LEN};
/// let input = b"7n42DGM5Tflk9n8mt7Fhc7";
/// let buf: [u8; BASE62_LEN] = input.try_into().unwrap();
/// let decoded = decode_fixed(&buf).unwrap();
/// assert_eq!(decoded, u128::MAX);
/// ```
pub fn decode_fixed(buf: &[u8; BASE62_LEN]) -> Result<u128, DecodeError> {
    let mut result = 0u128;

    for (position, &byte) in buf.iter().enumerate() {
        // Use lookup table to quickly get character value
        let value = DECODE_LUT[byte as usize];
        if value == 0xFF {
            return Err(DecodeError::InvalidCharacter { byte, position });
        }

        // Safely accumulate result, check for overflow
        result = result
            .checked_mul(BASE as u128)
            .and_then(|r| r.checked_add(value as u128))
            .ok_or(DecodeError::ArithmeticOverflow)?;
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let test_values = [0u128, 1, 61, 62, 3843, u64::MAX as u128, u128::MAX / 2, u128::MAX];

        for &value in &test_values {
            let mut buf = [0u8; BASE62_LEN];
            encode_fixed(value, &mut buf);
            let decoded = decode_fixed(&buf).unwrap();
            assert_eq!(value, decoded, "Failed for value: {}", value);
        }
    }

    #[test]
    fn test_invalid_decode() {
        let mut buf = [b'0'; BASE62_LEN];
        buf[0] = b'!'; // Invalid character

        match decode_fixed(&buf) {
            Err(DecodeError::InvalidCharacter { byte: b'!', position: 0 }) => {}
            other => panic!("Expected InvalidCharacter error, got: {:?}", other),
        }
    }
}
