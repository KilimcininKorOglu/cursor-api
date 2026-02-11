use alloc::borrow::Cow;

#[allow(private_bounds)]
#[inline]
pub fn string_from_utf8<V: StringFrom>(v: V) -> Option<String> {
    if byte_str::is_valid_utf8(v.as_bytes()) {
        Some(unsafe { String::from_utf8_unchecked(v.into_vec()) })
    } else {
        None
    }
}

trait StringFrom: Sized {
    fn as_bytes(&self) -> &[u8];
    fn into_vec(self) -> Vec<u8>;
}

impl StringFrom for &[u8] {
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] { self }
    #[inline(always)]
    fn into_vec(self) -> Vec<u8> { self.to_vec() }
}

impl StringFrom for Cow<'_, [u8]> {
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] { self }
    #[inline(always)]
    fn into_vec(self) -> Vec<u8> { self.into_owned() }
}

// mod private {
//     pub trait Sealed: Sized {}

//     impl Sealed for &[u8] {}
//     impl Sealed for super::Cow<'_, [u8]> {}
// }

/// Check if there's a space after the first delimiter in JSON fragment
///
/// # Rules
/// - Find first `:` or `,` not inside string
/// - Check if immediately followed by space (0x20)
/// - Do not validate JSON format correctness
///
/// # Examples
/// ```
/// assert_eq!(has_space_after_separator(b"{\"a\": 1}"), true);
/// assert_eq!(has_space_after_separator(b"{\"a\":1}"), false);
/// assert_eq!(has_space_after_separator(b"\"no separator\""), false);
/// ```
pub const fn has_space_after_separator(json: &[u8]) -> bool {
    let mut in_string = false;
    let mut i = 0;

    while i < json.len() {
        let byte = json[i];

        if in_string {
            if byte == b'\\' {
                // Skip escape characters (avoid \" being mistaken for string end)
                i += 2;
                continue;
            }
            if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b':' | b',' => {
                    // Found delimiter, check next byte
                    return i + 1 < json.len() && json[i + 1] == b' ';
                }
                _ => {}
            }
        }

        i += 1;
    }

    false
}
