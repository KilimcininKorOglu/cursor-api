use alloc::borrow::Cow;

use super::tokenizer::Token;

#[derive(Debug)]
pub enum Pattern {
    // Special handling
    #[allow(clippy::upper_case_acronyms)]
    GPT,
    O(u8), // O1, O3, O4

    // Version number
    Version(Cow<'static, str>), // 3.5 Or v3.1

    // Date related (in parentheses)
    Date(Cow<'static, str>),  // 2024-04-09 Or 05-28
    DateMarker(&'static str), // latest, legacy (Time marker)

    // Regular word
    Word(&'static str),
}

pub struct ParseResult {
    pub main_parts: Vec<Pattern>,
    pub date_parts: Vec<Pattern>, // Only date related items go in parentheses
}

#[inline(always)]
pub fn parse_patterns(tokens: Vec<Token>) -> ParseResult {
    // Pre-allocate: most tokens become patterns, date parts usually fewer
    let mut main_parts = Vec::with_capacity(tokens.len());
    let mut date_parts = Vec::with_capacity(2); // Usually at most 1-2 date related items
    let mut i = 0;

    while i < tokens.len() {
        let token = &tokens[i];

        // Fast path: judge by first character
        match token.meta.first_char {
            b'g' if token.meta.len == 3 => {
                // May be gpt
                if token.content == "gpt" {
                    main_parts.push(Pattern::GPT);
                    i += 1;
                    continue;
                }
            }
            b'o' if token.meta.len == 2 => {
                // May be o1, o3, o4
                if let Some(&digit) = token.content.as_bytes().get(1)
                    && matches!(digit, b'1' | b'3' | b'4')
                {
                    main_parts.push(Pattern::O(digit - b'0'));
                    i += 1;
                    continue;
                }
            }
            b'v' | b'r' | b'k' if token.meta.len >= 2 => {
                // Version number pattern v3.1, r1, k2
                if is_version_pattern(token.content) {
                    main_parts.push(Pattern::Version(capitalize_first(token.content)));
                    i += 1;
                    continue;
                }
            }
            b'l' if token.meta.len == 6 => {
                // latest, legacy - as date marker
                if token.content == "latest" || token.content == "legacy" {
                    date_parts.push(Pattern::DateMarker(token.content));
                    i += 1;
                    continue;
                }
            }
            _ => {}
        }

        // Number handling
        if token.meta.is_digit_only {
            // Single digit version number merge
            if token.meta.digit_count == 1 && i + 1 < tokens.len() {
                let next = &tokens[i + 1];
                if next.meta.is_digit_only && next.meta.digit_count == 1 {
                    // Pre-allocate exact length: digit1 + '.' + digit2
                    let mut version = String::with_capacity(3);
                    version.push_str(token.content);
                    version.push('.');
                    version.push_str(next.content);
                    main_parts.push(Pattern::Version(Cow::Owned(version)));
                    i += 2;
                    continue;
                }
            }

            // Date detection (4-digit or 2-digit numbers)
            if (token.meta.digit_count == 4 || token.meta.digit_count == 2)
                && let Some(date) = try_parse_date(&tokens, i)
            {
                date_parts.push(Pattern::Date(date));
                // Update i value based on date length
                i = update_index_for_date(&tokens, i);
                continue;
            }

            // Other numbers as regular words
            main_parts.push(Pattern::Word(token.content));
            i += 1;
            continue;
        }

        // Version number with dot - borrow directly, no need to allocate
        if token.meta.has_dot {
            main_parts.push(Pattern::Version(Cow::Borrowed(token.content)));
            i += 1;
            continue;
        }

        // All other words as main parts
        main_parts.push(Pattern::Word(token.content));
        i += 1;
    }

    ParseResult { main_parts, date_parts }
}

#[inline(always)]
fn is_version_pattern(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 2 {
        return false;
    }

    // v3.1, r1, k2 etc
    matches!(bytes[0], b'v' | b'r' | b'k')
        && bytes[1..].iter().all(|&b| b.is_ascii_digit() || b == b'.')
}

#[inline(always)]
fn try_parse_date(tokens: &[Token], start: usize) -> Option<Cow<'static, str>> {
    let token = &tokens[start];

    // YYYY-MM-DD (must check first, because YYYY is also 4 digits)
    if token.meta.digit_count == 4 && start + 2 < tokens.len() {
        let next1 = &tokens[start + 1];
        let next2 = &tokens[start + 2];
        if next1.meta.is_digit_only
            && next1.meta.digit_count == 2
            && next2.meta.is_digit_only
            && next2.meta.digit_count == 2
        {
            // Pre-allocate exact length: 4 + '-' + 2 + '-' + 2 = 10
            let mut date = String::with_capacity(10);
            date.push_str(token.content);
            date.push('-');
            date.push_str(next1.content);
            date.push('-');
            date.push_str(next2.content);
            return Some(Cow::Owned(date));
        }
    }

    // MMDD -> MM-DD (e.g. 0528)
    // Only handle when 4-digit number looks like MMDD format (first two digits <= 12)
    if token.meta.digit_count == 4 {
        let bytes = token.content.as_bytes();
        // Check if may be month (01-12)
        let month = (bytes[0] - b'0') * 10 + (bytes[1] - b'0');
        if (1..=12).contains(&month) {
            // Pre-allocate exact length: 2 + '-' + 2 = 5
            let mut date = String::with_capacity(5);
            date.push_str(&token.content[0..2]);
            date.push('-');
            date.push_str(&token.content[2..4]);
            return Some(Cow::Owned(date));
        }
    }

    // MM-DD
    if token.meta.digit_count == 2 && start + 1 < tokens.len() {
        let next = &tokens[start + 1];
        if next.meta.is_digit_only && next.meta.digit_count == 2 {
            // Pre-allocate exact length: 2 + '-' + 2 = 5
            let mut date = String::with_capacity(5);
            date.push_str(token.content);
            date.push('-');
            date.push_str(next.content);
            return Some(Cow::Owned(date));
        }
    }

    None
}

#[inline(always)]
fn update_index_for_date(tokens: &[Token], start: usize) -> usize {
    let token = &tokens[start];

    // MMDD or individual date components
    if token.meta.digit_count == 4 || token.meta.digit_count == 2 {
        // Check if it is YYYY-MM-DD
        if token.meta.digit_count == 4
            && start + 2 < tokens.len()
            && tokens[start + 1].meta.is_digit_only
            && tokens[start + 1].meta.digit_count == 2
            && tokens[start + 2].meta.is_digit_only
            && tokens[start + 2].meta.digit_count == 2
        {
            return start + 3;
        }
        // MM-DD
        if token.meta.digit_count == 2
            && start + 1 < tokens.len()
            && tokens[start + 1].meta.is_digit_only
            && tokens[start + 1].meta.digit_count == 2
        {
            return start + 2;
        }
    }

    start + 1
}

#[inline(always)]
fn capitalize_first(s: &'static str) -> Cow<'static, str> {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return Cow::Borrowed(s);
    }

    let first_byte = bytes[0];

    // Fast path: already uppercase
    if first_byte.is_ascii_uppercase() {
        return Cow::Borrowed(s);
    }

    // Need convert: for ASCII lowercase letters
    if first_byte.is_ascii_lowercase() {
        // Pre-allocate exact length
        let mut result = String::with_capacity(s.len());
        result.push((first_byte - b'a' + b'A') as char);
        result.push_str(&s[1..]);
        return Cow::Owned(result);
    }

    // 非 ASCII Or非字母，保持原样
    Cow::Borrowed(s)
}
