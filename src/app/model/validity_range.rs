//! Provides data structures and related operations for time validity ranges

use std::num::ParseIntError;

/// Represents a validity range, with two u32 values representing start and end times.
///
/// This struct uses transparent memory layout, implementing 8-byte size through [u32; 2].
/// Supports parsing from strings, e.g., "60" represents range 60-60, "3600-86400" represents closed interval from 3600 to 86400.
#[repr(transparent)]
pub struct ValidityRange {
    range: [u32; 2], // range[0] is start, range[1] is end
}

// Verify memory layout constraints
const _: [u8; 8] = [0; ::core::mem::size_of::<ValidityRange>()]; // Ensure size is 8 bytes

impl ValidityRange {
    /// Create a new validity range instance
    ///
    /// # Parameters
    ///
    /// * `start` - Start value of the range
    /// * `end` - End value of the range
    ///
    /// # Example
    ///
    /// ```
    /// let range = ValidityRange::new(60, 3600);
    /// ```
    #[inline]
    pub const fn new(start: u32, end: u32) -> Self {
        ValidityRange {
            range: [start, end],
        }
    }

    /// Get the start value of the range
    #[inline(always)]
    pub const fn start(&self) -> u32 { self.range[0] }

    /// Get the end value of the range
    #[inline(always)]
    pub const fn end(&self) -> u32 { self.range[1] }

    /// Check if a given value is within the validity range
    ///
    /// # Parameters
    ///
    /// * `value` - Value to check
    ///
    /// # Return value
    ///
    /// Returns true if value is within range (including boundary values), otherwise returns false
    ///
    /// # Example
    ///
    /// ```
    /// let range = ValidityRange::new(60, 3600);
    /// assert!(range.is_valid(60));
    /// assert!(range.is_valid(3600));
    /// assert!(range.is_valid(1800));
    /// assert!(!range.is_valid(59));
    /// assert!(!range.is_valid(3601));
    /// ```
    #[inline]
    pub const fn is_valid(&self, value: u32) -> bool {
        value >= self.start() && value <= self.end()
    }

    /// Parse validity range from string
    ///
    /// Supports two formats:
    /// - "N" represents single-point range N-N
    /// - "N-M" represents range from N to M
    ///
    /// # Parameters
    ///
    /// * `s` - String to parse
    ///
    /// # Return value
    ///
    /// Returns `Ok(ValidityRange)` on successful parse, `Err` with error type on failure
    ///
    /// # Example
    ///
    /// ```
    /// let range1 = ValidityRange::from_str("60").unwrap();
    /// assert_eq!(range1.start(), 60);
    /// assert_eq!(range1.end(), 60);
    ///
    /// let range2 = ValidityRange::from_str("3600-86400").unwrap();
    /// assert_eq!(range2.start(), 3600);
    /// assert_eq!(range2.end(), 86400);
    /// ```
    pub fn from_str(s: &str) -> Result<Self, ParseIntError> {
        if let Some((start_str, end_str)) = s.split_once('-') {
            let start = start_str.parse::<u32>()?;
            let end = end_str.parse::<u32>()?;

            Ok(ValidityRange::new(start, end))
        } else {
            // Format: "value" (represents value-value)
            let value = s.parse::<u32>()?;
            Ok(ValidityRange::new(value, value))
        }
    }
}

/// Implement Display trait for formatting output
///
/// For same start and end values, display only one number;
/// For different values, display as "start-end" format.
impl std::fmt::Display for ValidityRange {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let start = self.start();
        let end = self.end();

        if start == end {
            write!(f, "{start}")
        } else {
            write!(f, "{start}-{end}")
        }
    }
}

/// Implement Debug trait, provide more detailed formatting output
impl std::fmt::Debug for ValidityRange {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ValidityRange({}-{})", self.start(), self.end())
    }
}

/// Implement FromStr trait, support parsing from string
///
/// This allows directly using `str.parse()` method to parse string to ValidityRange
impl std::str::FromStr for ValidityRange {
    type Err = ParseIntError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> { ValidityRange::from_str(s) }
}

/// Unit tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let range = ValidityRange::new(60, 3600);
        assert_eq!(range.start(), 60);
        assert_eq!(range.end(), 3600);
    }

    #[test]
    fn test_is_valid() {
        let range = ValidityRange::new(60, 3600);
        assert!(range.is_valid(60));
        assert!(range.is_valid(3600));
        assert!(range.is_valid(1800));
        assert!(!range.is_valid(59));
        assert!(!range.is_valid(3601));
    }

    #[test]
    fn test_from_str_single() {
        let range = ValidityRange::from_str("60").unwrap();
        assert_eq!(range.start(), 60);
        assert_eq!(range.end(), 60);
    }

    #[test]
    fn test_from_str_range() {
        let range = ValidityRange::from_str("3600-86400").unwrap();
        assert_eq!(range.start(), 3600);
        assert_eq!(range.end(), 86400);
    }

    #[test]
    fn test_from_str_invalid() {
        assert!(ValidityRange::from_str("abc").is_err());
        assert!(ValidityRange::from_str("123-abc").is_err());
        assert!(ValidityRange::from_str("abc-123").is_err());
    }

    #[test]
    fn test_display() {
        let range1 = ValidityRange::new(60, 60);
        assert_eq!(format!("{range1}"), "60");

        let range2 = ValidityRange::new(3600, 86400);
        assert_eq!(format!("{range2}"), "3600-86400");
    }

    #[test]
    fn test_debug() {
        let range = ValidityRange::new(60, 3600);
        assert_eq!(format!("{range:?}"), "ValidityRange(60-3600)");
    }

    #[test]
    fn test_parse() {
        let range: Result<ValidityRange, _> = "60".parse();
        assert!(range.is_ok());
        let range = range.unwrap();
        assert_eq!(range.start(), 60);
        assert_eq!(range.end(), 60);
    }
}
