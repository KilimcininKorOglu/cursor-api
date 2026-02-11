//! Cursor version information management module
//!
//! This module uses ManuallyInit to store version information, this design considers the following factors:
//! 1. Version information only needs to be initialized once during program lifecycle
//! 2. Zero-cost access: no atomic operations or runtime checks
//! 3. Conforms to single-threaded initialization, multi-threaded read-only pattern
//!
//! # Safety
//!
//! Safety guarantees:
//! - Initialization function `initialize_cursor_version` must be called in single-threaded environment at program startup
//! - `initialize_cursor_version` must be called exactly once
//! - After initialization, all accesses are read-only (returns copies via clone())

use manually_init::ManuallyInit;

// Define all constants
crate::define_typed_constants! {
    &'static str => {
        /// Default client version number
        DEFAULT_CLIENT_VERSION = "2.0.0",
        /// Environment variable name: Cursor client version
        ENV_CURSOR_CLIENT_VERSION = "CURSOR_CLIENT_VERSION",
        /// Chrome version information
        CHROME_VERSION_INFO = " Chrome/138.0.7204.251 Electron/37.7.0 Safari/537.36",
        /// User-Agent prefix
        UA_PREFIX = cfg_select! {
            target_os = "windows" => {"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Cursor/"}
            target_os = "macos" => {"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Cursor/"}
            target_os = "linux" => {"Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Cursor/"}
        },
        /// Default User-Agent
        DEFAULT_UA = cfg_select! {
            target_os = "windows" => {"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Cursor/2.0.0 Chrome/138.0.7204.251 Electron/37.7.0 Safari/537.36"}
            target_os = "macos" => {"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Cursor/2.0.0 Chrome/138.0.7204.251 Electron/37.7.0 Safari/537.36"}
            target_os = "linux" => {"Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Cursor/2.0.0 Chrome/138.0.7204.251 Electron/37.7.0 Safari/537.36"}
        },
    }

    usize => {
        /// Version string minimum length
        VERSION_MIN_LENGTH = 5,
        /// Version string maximum length
        VERSION_MAX_LENGTH = 32,
    }

    u8 => {
        /// Maximum number of digits for each part in version number
        VERSION_PART_MAX_DIGITS = 4,
        /// Expected number of dots in version number
        VERSION_DOT_COUNT = 2,
    }
}

/// Client version HeaderValue
static CLIENT_VERSION: ManuallyInit<http::header::HeaderValue> = ManuallyInit::new();

/// Cursor User-Agent HeaderValue
static HEADER_VALUE_UA_CURSOR_LATEST: ManuallyInit<http::header::HeaderValue> = ManuallyInit::new();

/// Get Cursor client version HeaderValue
///
/// # Safety
///
/// Caller must ensure `initialize_cursor_version` has been called.
#[inline(always)]
pub fn cursor_client_version() -> http::header::HeaderValue { CLIENT_VERSION.get().clone() }

#[inline(always)]
pub fn cursor_version() -> bytes::Bytes {
    use crate::common::model::HeaderValue;
    let value_ref: &'static HeaderValue = CLIENT_VERSION.get().into();
    value_ref.inner.clone()
}

/// Get Cursor user agent HeaderValue
///
/// # Safety
///
/// Caller must ensure `initialize_cursor_version` has been called.
#[inline(always)]
pub fn header_value_ua_cursor_latest() -> http::header::HeaderValue {
    HEADER_VALUE_UA_CURSOR_LATEST.get().clone()
}

/// Initialize Cursor version information
///
/// # Safety
///
/// This function must satisfy the following conditions:
/// 1. Called in single-threaded environment at program startup
/// 2. Can only be called once during entire program lifecycle
/// 3. Must be called before calling `cursor_client_version` or `header_value_ua_cursor_latest`
pub fn initialize_cursor_version() {
    use ::core::ops::Deref as _;

    let version =
        crate::common::utils::parse_from_env(ENV_CURSOR_CLIENT_VERSION, DEFAULT_CLIENT_VERSION);

    // Verify version format
    validate_version_string(&version);

    let version_header = match http::header::HeaderValue::from_str(&version) {
        Ok(header) => header,
        Err(_) => {
            __cold_path!();
            __eprintln!("Error: Invalid version string for HTTP header");
            // Use default version
            const { http::header::HeaderValue::from_static(DEFAULT_CLIENT_VERSION) }
        }
    };

    // Build User-Agent string
    let ua_string = [UA_PREFIX, version.deref(), CHROME_VERSION_INFO].concat();

    let ua_header = match http::header::HeaderValue::from_str(&ua_string) {
        Ok(header) => header,
        Err(_) => {
            __cold_path!();
            __eprintln!("Error: Invalid user agent string for HTTP header");
            // Use default UA
            const { http::header::HeaderValue::from_static(DEFAULT_UA) }
        }
    };

    CLIENT_VERSION.init(version_header);
    HEADER_VALUE_UA_CURSOR_LATEST.init(ua_header);
}

/// Check whether version string conforms to VSCode/Cursor version format
///
/// Expected format: `major.minor.patch`
/// Example: `1.0.0`, `1.95.3`
///
/// # Returns
///
/// Returns `true` if version format is valid, otherwise returns `false`
#[inline]
pub const fn is_valid_version_format(version: &str) -> bool {
    // Fast path: check basic length requirements
    if version.len() < VERSION_MIN_LENGTH || version.len() > VERSION_MAX_LENGTH {
        return false;
    }

    let bytes = version.as_bytes();
    let mut dot_count = 0u8;
    let mut digit_count = 0u8;
    let mut i = 0;

    // Parse major.minor.patch parts
    while i < bytes.len() {
        match bytes[i] {
            b'0'..=b'9' => {
                digit_count += 1;
                // Prevent digit part from being too long
                if digit_count > VERSION_PART_MAX_DIGITS {
                    return false;
                }
            }
            b'.' => {
                // Must have digits before dot
                if digit_count == 0 {
                    return false;
                }
                dot_count += 1;
                if dot_count > VERSION_DOT_COUNT {
                    return false;
                }
                digit_count = 0;
            }
            _ => return false,
        }
        i += 1;
    }

    // Must have exactly two dots and last part must have digits
    dot_count == VERSION_DOT_COUNT && digit_count > 0
}

/// Verify and warn about invalid version strings
///
/// If version string does not conform to format, print warning message but do not terminate program
#[inline]
pub fn validate_version_string(version: &str) {
    if !is_valid_version_format(version) {
        __cold_path!();
        eprint!(
            "Warning: Invalid version format '{version}'. Expected format: major.minor.patch (e.g., 1.0.0)\n"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_version_formats() {
        assert!(is_valid_version_format("1.0.0"));
        assert!(is_valid_version_format("1.95.3"));
        assert!(is_valid_version_format("10.20.30"));
        assert!(is_valid_version_format("1234.5678.9012"));
    }

    #[test]
    fn test_invalid_version_formats() {
        assert!(!is_valid_version_format("1.0"));
        assert!(!is_valid_version_format("1.0.0.0"));
        assert!(!is_valid_version_format("v1.0.0"));
        assert!(!is_valid_version_format(".1.0.0"));
        assert!(!is_valid_version_format("1..0"));
        assert!(!is_valid_version_format(""));
        assert!(!is_valid_version_format("1.0."));
        assert!(!is_valid_version_format("10000.0.0")); // exceeds 4 digits
        assert!(!is_valid_version_format("1")); // too short
        assert!(!is_valid_version_format(&"1.0.".repeat(20))); // too long
    }
}
