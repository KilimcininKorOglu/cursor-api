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

        // Use last four bytes to restore timestamp
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

    // Length gating
    if len != 72 && len != 137 {
        return false;
    }

    // Single pass to complete all character validation
    for (i, &b) in bytes.iter().enumerate() {
        let valid = match (len, i) {
            // Generic character validation (exclude illegal characters)
            (_, _) if !b.is_ascii_alphanumeric() && b != b'/' && b != b'-' && b != b'_' => false,

            // Format validation
            (72, 0..=7) => true, // Timestamp part (verified by extract_time_ks)
            (72, 8..=71) => b.is_ascii_hexdigit(),

            (137, 0..=7) => true,                     // Timestamp
            (137, 8..=71) => b.is_ascii_hexdigit(),   // Device hash
            (137, 72) => b == b'/',                   // Separator (index 72 is the 73rd character)
            (137, 73..=136) => b.is_ascii_hexdigit(), // MAC hash

            _ => unreachable!(),
        };

        if !valid {
            return false;
        }
    }

    // Unified timestamp validation (no need for layering)
    let time_valid = extract_time_ks(unsafe { checksum.get_unchecked(..8) }).is_some();

    time_valid
}
