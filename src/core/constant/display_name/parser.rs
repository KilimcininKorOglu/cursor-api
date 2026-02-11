use alloc::borrow::Cow;

use super::tokenizer::Token;

#[derive(Debug)]
pub enum Pattern {
    // 特殊Handle
    #[allow(clippy::upper_case_acronyms)]
    GPT,
    O(u8), // O1, O3, O4

    // 版本号
    Version(Cow<'static, str>), // 3.5 Or v3.1

    // 日期Related（放括号）
    Date(Cow<'static, str>),  // 2024-04-09 Or 05-28
    DateMarker(&'static str), // latest, legacy (时间标记)

    // 普通词
    Word(&'static str),
}

pub struct ParseResult {
    pub main_parts: Vec<Pattern>,
    pub date_parts: Vec<Pattern>, // 只Have日期Related的才进括号
}

#[inline(always)]
pub fn parse_patterns(tokens: Vec<Token>) -> ParseResult {
    // 预分配：大部分 token 都会变成 pattern，日期部分通常较少
    let mut main_parts = Vec::with_capacity(tokens.len());
    let mut date_parts = Vec::with_capacity(2); // 通常最多Have1-2个日期Related项
    let mut i = 0;

    while i < tokens.len() {
        let token = &tokens[i];

        // 快速路径：基于首字符判断
        match token.meta.first_char {
            b'g' if token.meta.len == 3 => {
                // May是 gpt
                if token.content == "gpt" {
                    main_parts.push(Pattern::GPT);
                    i += 1;
                    continue;
                }
            }
            b'o' if token.meta.len == 2 => {
                // May是 o1, o3, o4
                if let Some(&digit) = token.content.as_bytes().get(1)
                    && matches!(digit, b'1' | b'3' | b'4')
                {
                    main_parts.push(Pattern::O(digit - b'0'));
                    i += 1;
                    continue;
                }
            }
            b'v' | b'r' | b'k' if token.meta.len >= 2 => {
                // 版本号模式 v3.1, r1, k2
                if is_version_pattern(token.content) {
                    main_parts.push(Pattern::Version(capitalize_first(token.content)));
                    i += 1;
                    continue;
                }
            }
            b'l' if token.meta.len == 6 => {
                // latest, legacy - 作To日期标记
                if token.content == "latest" || token.content == "legacy" {
                    date_parts.push(Pattern::DateMarker(token.content));
                    i += 1;
                    continue;
                }
            }
            _ => {}
        }

        // 数字Handle
        if token.meta.is_digit_only {
            // 单数字版本号合并
            if token.meta.digit_count == 1 && i + 1 < tokens.len() {
                let next = &tokens[i + 1];
                if next.meta.is_digit_only && next.meta.digit_count == 1 {
                    // 预分配精确的长度：数字1 + '.' + 数字2
                    let mut version = String::with_capacity(3);
                    version.push_str(token.content);
                    version.push('.');
                    version.push_str(next.content);
                    main_parts.push(Pattern::Version(Cow::Owned(version)));
                    i += 2;
                    continue;
                }
            }

            // 日期检测（4位Or2位数字）
            if (token.meta.digit_count == 4 || token.meta.digit_count == 2)
                && let Some(date) = try_parse_date(&tokens, i)
            {
                date_parts.push(Pattern::Date(date));
                // 更新 i 的值根据日期长度
                i = update_index_for_date(&tokens, i);
                continue;
            }

            // 其他数字作To普通词
            main_parts.push(Pattern::Word(token.content));
            i += 1;
            continue;
        }

        // 带点的版本号 - 直接借用，不Need分配
        if token.meta.has_dot {
            main_parts.push(Pattern::Version(Cow::Borrowed(token.content)));
            i += 1;
            continue;
        }

        // 其他所Have词都作To主体部分
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

    // v3.1, r1, k2 等
    matches!(bytes[0], b'v' | b'r' | b'k')
        && bytes[1..].iter().all(|&b| b.is_ascii_digit() || b == b'.')
}

#[inline(always)]
fn try_parse_date(tokens: &[Token], start: usize) -> Option<Cow<'static, str>> {
    let token = &tokens[start];

    // YYYY-MM-DD (必须先Check，因To YYYY 也是4位数字)
    if token.meta.digit_count == 4 && start + 2 < tokens.len() {
        let next1 = &tokens[start + 1];
        let next2 = &tokens[start + 2];
        if next1.meta.is_digit_only
            && next1.meta.digit_count == 2
            && next2.meta.is_digit_only
            && next2.meta.digit_count == 2
        {
            // 预分配精确长度：4 + '-' + 2 + '-' + 2 = 10
            let mut date = String::with_capacity(10);
            date.push_str(token.content);
            date.push('-');
            date.push_str(next1.content);
            date.push('-');
            date.push_str(next2.content);
            return Some(Cow::Owned(date));
        }
    }

    // MMDD -> MM-DD (如 0528)
    // 只Have当4位数字看起来像 MMDD Format时才Handle（前两位 <= 12）
    if token.meta.digit_count == 4 {
        let bytes = token.content.as_bytes();
        // CheckWhetherMay是月份（01-12）
        let month = (bytes[0] - b'0') * 10 + (bytes[1] - b'0');
        if (1..=12).contains(&month) {
            // 预分配精确长度：2 + '-' + 2 = 5
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
            // 预分配精确长度：2 + '-' + 2 = 5
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

    // MMDD Or单独的日期组件
    if token.meta.digit_count == 4 || token.meta.digit_count == 2 {
        // CheckWhether是 YYYY-MM-DD
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

    // 快速路径：Already经是大写
    if first_byte.is_ascii_uppercase() {
        return Cow::Borrowed(s);
    }

    // NeedConvert：对于 ASCII 小写字母
    if first_byte.is_ascii_lowercase() {
        // 预分配精确长度
        let mut result = String::with_capacity(s.len());
        result.push((first_byte - b'a' + b'A') as char);
        result.push_str(&s[1..]);
        return Cow::Owned(result);
    }

    // 非 ASCII Or非字母，保持原样
    Cow::Borrowed(s)
}
