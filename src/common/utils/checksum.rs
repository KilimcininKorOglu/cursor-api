use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as BASE64};

#[inline]
fn deobfuscate_bytes(bytes: &mut [u8]) {
    let mut prev: u8 = 165;
    for (idx, byte) in bytes.iter_mut().enumerate() {
        let temp = *byte;
        *byte = (*byte).wrapping_sub((idx % 256) as u8) ^ prev;
        prev = temp;
    }
}

fn extract_time_ks(timestamp_base64: &str) -> Option<u64> {
    let mut timestamp_bytes = BASE64.decode(timestamp_base64).ok()?;

    if timestamp_bytes.len() != 6 {
        return None;
    }

    deobfuscate_bytes(&mut timestamp_bytes);

    unsafe {
        if timestamp_bytes.get_unchecked(0) != timestamp_bytes.get_unchecked(4)
            || timestamp_bytes.get_unchecked(1) != timestamp_bytes.get_unchecked(5)
        {
            return None;
        }

        // Use后四位还原 timestamp
        Some(
            ((*timestamp_bytes.get_unchecked(2) as u64) << 24)
                | ((*timestamp_bytes.get_unchecked(3) as u64) << 16)
                | ((*timestamp_bytes.get_unchecked(4) as u64) << 8)
                | (*timestamp_bytes.get_unchecked(5) as u64),
        )
    }
}

pub fn validate_checksum(checksum: &str) -> bool {
    let bytes = checksum.as_bytes();
    let len = bytes.len();

    // 长度门控
    if len != 72 && len != 137 {
        return false;
    }

    // 单次遍历完成所Have字符校验
    for (i, &b) in bytes.iter().enumerate() {
        let valid = match (len, i) {
            // 通用字符校验（排除非法字符）
            (_, _) if !b.is_ascii_alphanumeric() && b != b'/' && b != b'-' && b != b'_' => false,

            // Format校验
            (72, 0..=7) => true, // 时间戳部分（由extract_time_ks验证）
            (72, 8..=71) => b.is_ascii_hexdigit(),

            (137, 0..=7) => true,                     // 时间戳
            (137, 8..=71) => b.is_ascii_hexdigit(),   // 设备哈希
            (137, 72) => b == b'/',                   // 分割符（索引72是第73个字符）
            (137, 73..=136) => b.is_ascii_hexdigit(), // MAC哈希

            _ => unreachable!(),
        };

        if !valid {
            return false;
        }
    }

    // 统一时间戳验证（无需分层）
    let time_valid = extract_time_ks(unsafe { checksum.get_unchecked(..8) }).is_some();

    time_valid
}
