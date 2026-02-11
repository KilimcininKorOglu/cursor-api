use alloc::borrow::Cow;

use super::parser::{ParseResult, Pattern};

#[inline(always)]
pub fn format_output(result: ParseResult) -> String {
    let mut output = String::with_capacity(64);

    // Format main parts
    for (i, pattern) in result.main_parts.iter().enumerate() {
        if i > 0 {
            output.push(' ');
        }

        match pattern {
            Pattern::GPT => output.push_str("GPT"),
            Pattern::O(n) => {
                output.push('O');
                output.push((b'0' + n) as char);
            }
            Pattern::Version(v) => output.push_str(v.as_ref()),
            Pattern::Word(w) => output.push_str(capitalize_word(w).as_ref()),
            _ => {} // Date related should not be in main parts
        }
    }

    // Format date related parts (in parentheses)
    for date_item in result.date_parts.iter() {
        output.push_str(" (");
        match date_item {
            Pattern::Date(d) => output.push_str(d.as_ref()),
            Pattern::DateMarker(m) => output.push_str(m), // latest, legacy
            _ => unreachable!(),                          // Other should not be in date parts
        }
        output.push(')');
    }

    output
}

#[inline(always)]
fn capitalize_word(word: &str) -> Cow<'_, str> {
    // Special case handling - need complete replacement
    if word == "default" {
        return Cow::Borrowed("Default");
    }

    let bytes = word.as_bytes();
    if bytes.is_empty() {
        return Cow::Borrowed(word);
    }

    // Quick check if first character is already uppercase
    let first_byte = bytes[0];

    // Fast path for ASCII characters
    if first_byte.is_ascii() {
        if first_byte.is_ascii_uppercase() {
            // Already uppercase, return directly
            return Cow::Borrowed(word);
        }

        if first_byte.is_ascii_lowercase() {
            // ASCII lowercase to uppercase, direct byte operation
            let mut result = String::with_capacity(word.len());
            result.push((first_byte - b'a' + b'A') as char);
            result.push_str(&word[1..]);
            return Cow::Owned(result);
        }

        // ASCII but not letter (e.g. digit), keep as is
        return Cow::Borrowed(word);
    }

    // Handle non-ASCII characters (though rare in AI model names)
    let mut chars = word.chars();
    match chars.next() {
        None => Cow::Borrowed(word),
        Some(first) if first.is_uppercase() => Cow::Borrowed(word),
        Some(first) => {
            // Pre-allocate enough space (assume worst case uppercase doubles length)
            let mut result = String::with_capacity(word.len() + 4);
            for ch in first.to_uppercase() {
                result.push(ch);
            }
            result.push_str(chars.as_str());
            Cow::Owned(result)
        }
    }
}
