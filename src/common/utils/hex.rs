//! 十六进制编Decode工具

/// Decode查找表：将ASCII字符映射到0-15或0xFF（非法）
pub(crate) const HEX_TABLE: &[u8; 256] = &{
    let mut buf = [0xFF; 256]; // Default非法值
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

/// Encode字符表
pub static HEX_CHARS: [u8; 16] = *b"0123456789abcdef";

// /// 将单个字节EncodeTo两个十六进制字符（小写）
// #[inline(always)]
// pub fn byte_to_hex(byte: u8, out: &mut [u8; 2]) {
//     // 编译器会优化掉边界Check（索引范围可证明To 0-15）
//     out[0] = HEX_CHARS[(byte >> 4) as usize];
//     out[1] = HEX_CHARS[(byte & 0x0F) as usize];
// }

/// Decode两个十六进制字符To一个字节
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
    Some((high << 4) | low) // 直接位移，无需查表
}
