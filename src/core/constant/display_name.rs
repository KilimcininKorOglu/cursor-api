use parking_lot::Mutex;
use manually_init::ManuallyInit;

use crate::app::constant::EMPTY_STRING;

type HashMap<K, V> = hashbrown::HashMap<K, V, ahash::RandomState>;

static DISPLAY_NAME_CACHE: ManuallyInit<Mutex<HashMap<&'static str, &'static str>>> =
    ManuallyInit::new();

pub fn init_display_name_cache() {
    DISPLAY_NAME_CACHE.init(Mutex::new(HashMap::default()))
}

/// Calculate display name of AI model identifier。
///
/// # ConvertRules
///
/// 1. **Version number merge**：Single digit-single digit → decimal version number (e.g. `3-5` → `3.5`)
/// 2. **Date retention**：Date format displayed in parentheses
///    - `YYYY-MM-DD` Format：`2024-04-09` → `(2024-04-09)`
///    - `MM-DD` Format：`03-25` → `(03-25)`  
///    - `MMDD` Format：`0528` → `(05-28)`
/// 3. **Time marker**：`latest` and `legacy` displayed in parentheses
/// 4. **Special prefix**：
///    - `gpt` → `GPT`
///    - `o1`/`o3`/`o4` → `O1`/`O3`/`O4`
/// 5. **Version marker**：`v`/`r`/`k` Version number starting with first letter capitalized (e.g. `v3.1` → `V3.1`)
/// 6. **Separator convert**：Other `-` convert to empty format, each part first letter capitalized
///
/// # Arguments
///
/// * `identifier` - Original identifier string of AI model
///
/// # Returns
///
/// * `&'static str` - Formatted display name（Cache）
///
/// # Examples
///
/// ```
/// // Basic convert
/// assert_eq!(calculate_display_name("claude-3-5-sonnet"), "Claude 3.5 Sonnet");
/// assert_eq!(calculate_display_name("deepseek-v3"), "Deepseek V3");
///
/// // GPT special handle
/// assert_eq!(calculate_display_name("gpt-4o"), "GPT 4o");
/// assert_eq!(calculate_display_name("gpt-3.5-turbo"), "GPT 3.5 Turbo");
///
/// // Date handle（Put in parentheses）
/// assert_eq!(calculate_display_name("gpt-4-turbo-2024-04-09"), "GPT 4 Turbo (2024-04-09)");
/// assert_eq!(calculate_display_name("gemini-2.5-pro-exp-03-25"), "Gemini 2.5 Pro Exp (03-25)");
/// assert_eq!(calculate_display_name("deepseek-r1-0528"), "Deepseek R1 (05-28)");
///
/// // Time marker（Put in parentheses）
/// assert_eq!(calculate_display_name("gemini-2.5-pro-latest"), "Gemini 2.5 Pro (latest)");
/// assert_eq!(calculate_display_name("claude-4-opus-legacy"), "Claude 4 Opus (legacy)");
///
/// // O series
/// assert_eq!(calculate_display_name("o3-mini"), "O3 Mini");
///
/// // Boundary case
/// assert_eq!(calculate_display_name("version-10-beta"), "Version 10 Beta"); // 10 Not single digit
/// assert_eq!(calculate_display_name("model-1-test-9-case"), "Model 1 Test 9 Case"); // Single digits not adjacent
/// ```
pub fn calculate_display_name(identifier: &'static str) -> &'static str {
    if let Some(cached) = DISPLAY_NAME_CACHE.lock().get(identifier) {
        return cached;
    }

    let result = if identifier.is_empty() {
        EMPTY_STRING
    } else {
        crate::leak::intern(calculate_display_name_internal(identifier))
    };

    DISPLAY_NAME_CACHE.lock().insert(identifier, result);

    result
}

mod formatter;
mod parser;
mod tokenizer;

use formatter::format_output;
use parser::parse_patterns;
use tokenizer::tokenize;

#[inline(always)]
fn calculate_display_name_internal(identifier: &'static str) -> String {
    let tokens = tokenize(identifier);
    let patterns = parse_patterns(tokens);
    format_output(patterns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_output() {
        let test_cases = vec![
            // Basic test
            "",
            "default",
            "sonic",
            // GPT series
            "gpt",
            "gpt-4",
            "gpt-4o",
            "gpt-3.5-turbo",
            "gpt-4-turbo-2024-04-09",
            "gpt-5-high-fast",
            "gpt-5-mini",
            // O series
            "o1",
            "o3",
            "o3-mini",
            "o3-pro",
            "o4-mini",
            "o1-preview",
            // Claude series
            "claude-3-opus",
            "claude-3.5-sonnet",
            "claude-3-5-sonnet",
            "claude-4-opus-thinking",
            "claude-4.1-opus-thinking",
            "claude-3.7-sonnet-thinking",
            "claude-4-opus-legacy",
            "claude-4-opus-thinking-legacy",
            "claude-3-haiku-200k",
            "claude-3.5-sonnet-200k",
            // Gemini series
            "gemini-2.5-pro",
            "gemini-2.5-flash",
            "gemini-2.5-pro-latest",
            "gemini-2.5-pro-exp-03-25",
            "gemini-2.5-pro-preview-05-06",
            "gemini-2.0-flash-thinking-exp",
            "gemini-1.5-flash-500k",
            "gemini-2.5-pro-max",
            // Deepseek series
            "deepseek-v3",
            "deepseek-v3.1",
            "deepseek-r1",
            "deepseek-r1-0528",
            // Grok series
            "grok-2",
            "grok-3",
            "grok-3-beta",
            "grok-3-mini",
            "grok-4",
            "grok-4-0709",
            // Other models
            "cursor-small",
            "cursor-fast",
            "kimi-k2-instruct",
            "accounts/fireworks/models/kimi-k2-instruct",
            // Version number test
            "model-3-5",
            "model-3.5",
            "test-1-0",
            "version-10-beta",
            "model-10-5",
            "app-2.5-release",
            // Date test
            "release-2024-04-09",
            "update-03-25",
            "version-0528",
            "model-123",
            "test-12345",
            // Boundary case
            "model-1-2-3",
            "model-1-test-9-case",
            "model-fast-experimental-latest",
            "-start",
            "end-",
            "-",
            "a--b",
            "3-5",
            "2024",
            // Complex combination
            "gpt-4.5-preview",
            "claude-3.5-sonnet-200k",
            "gemini-1-5-flash-500k",
        ];

        println!("\n=== Parser Test Results ===\n");

        for identifier in test_cases {
            println!("Input: {:?}", identifier);

            let tokens = tokenize(identifier);
            println!("  Tokens: {:?}", tokens);

            let patterns = parse_patterns(tokens);
            println!("  Main parts: {:?}", patterns.main_parts);
            println!("  Date parts: {:?}", patterns.date_parts);

            let output = format_output(patterns);
            println!("  Output: {:?}", output);
            println!("  ---");
        }
    }

    #[test]
    fn test_tokenizer_details() {
        println!("\n=== Tokenizer Details ===\n");

        let test_cases = vec![
            "gpt-4-turbo-2024-04-09",
            "claude-3.5-sonnet-thinking",
            "deepseek-r1-0528",
            "gemini-2.5-pro-exp-03-25",
        ];

        for identifier in test_cases {
            println!("Input: {:?}", identifier);
            let tokens = tokenize(identifier);

            for (i, token) in tokens.iter().enumerate() {
                println!("  Token[{}]: {:?}", i, token);
                println!("    content: {:?}", token.content);
                println!("    meta: {{");
                println!("      is_digit_only: {}", token.meta.is_digit_only);
                println!("      digit_count: {}", token.meta.digit_count);
                println!("      has_dot: {}", token.meta.has_dot);
                println!(
                    "      first_char: {:?} ({})",
                    token.meta.first_char as char, token.meta.first_char
                );
                println!("      len: {}", token.meta.len);
                println!("    }}");
            }
            println!();
        }
    }

    #[test]
    fn test_pattern_recognition() {
        println!("\n=== Pattern Recognition ===\n");

        let special_cases = vec![
            ("Single digit merge", "model-3-5-test"),
            ("Version with dot", "v3.1-release"),
            ("Date YYYY-MM-DD", "version-2024-04-09"),
            ("Date MM-DD", "update-03-25"),
            ("Date MMDD", "release-0528"),
            ("Latest marker", "model-latest"),
            ("Legacy marker", "model-legacy"),
            ("Mixed", "gpt-4.5-turbo-latest-2024-04-09"),
        ];

        for (description, identifier) in special_cases {
            println!("{}: {:?}", description, identifier);

            let tokens = tokenize(identifier);
            let patterns = parse_patterns(tokens);

            println!("  Patterns breakdown:");
            for (i, pattern) in patterns.main_parts.iter().enumerate() {
                println!("    Main[{}]: {:?}", i, pattern);
            }
            for (i, pattern) in patterns.date_parts.iter().enumerate() {
                println!("    Date[{}]: {:?}", i, pattern);
            }

            let output = format_output(patterns);
            println!("  Final: {:?}", output);
            println!();
        }
    }

    #[test]
    fn test_edge_cases() {
        println!("\n=== Edge Cases ===\n");

        let edge_cases = vec![
            ("Empty", ""),
            ("Single hyphen", "-"),
            ("Double hyphen", "--"),
            ("Start hyphen", "-model"),
            ("End hyphen", "model-"),
            ("Multiple hyphens", "a---b"),
            ("Just numbers", "123"),
            ("Just dot", "."),
            ("Dot at start", ".model"),
            ("Dot at end", "model."),
            ("Multiple dots", "model...test"),
            ("Mixed separators", "model-1.5-test"),
        ];

        for (description, identifier) in edge_cases {
            println!("{}: {:?}", description, identifier);

            let tokens = tokenize(identifier);
            println!("  Tokens: {:?}", tokens);

            let patterns = parse_patterns(tokens);
            let output = format_output(patterns);
            println!("  Output: {:?}", output);
            println!();
        }
    }
}
