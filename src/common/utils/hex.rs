//! Hexadecimal encoding/decoding utilities

/// Decode lookup table: maps ASCII characters to 0-15 or 0xFF (invalid)
pub(crate) const HEX_TABLE: &[u8; 256] = &{
    let mut buf = [0xFF; 256]; // Default invalid value
    let mut i: u8 = 0;
    loop {
        buf[i as usize] = match i {
            b'0'..=b'9' => i - b'0',
            b'a'..=b'f' => i - b'a' + 10,
            b'A'..=b'F' => i - b'A' + 10,
            _ => 0xFF,
        };
        if i == 255 {
            break buf;
        }
        i += 1;
    }
};

/// Encode character table
pub static HEX_CHARS: [u8; 16] = *b"0123456789abcdef";

// /// Encode single byte to two hexadecimal characters (lowercase)
// #[inline(always)]
// pub fn byte_to_hex(byte: u8, out: &mut [u8; 2]) {
//     // Compiler will optimize away boundary checks (index range provably 0-15)
//     out[0] = HEX_CHARS[(byte >> 4) as usize];
//     out[1] = HEX_CHARS[(byte & 0x0F) as usize];
// }

/// Decode two hexadecimal characters to one byte
#[inline(always)]
pub const fn hex_to_byte(hi: u8, lo: u8) -> Option<u8> {
    let high = HEX_TABLE[hi as usize];
    if high == 0xFF {
        return None;
    }
    let low = HEX_TABLE[lo as usize];
    if low == 0xFF {
        return None;
    }
    Some((high << 4) | low) // Direct bit shift, no lookup needed
}
