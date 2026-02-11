//! # Duration Formatter
//!
//! A high-performance library for formatting time durations in various human-readable formats.
//!
//! This module provides flexible duration formatting with multiple output styles,
//! optimized for zero-allocation formatting.
//!
//! ## Features
//!
//! - Multiple formatting styles (compact, standard, detailed, ISO8601, etc.)
//! - Automatic format selection based on duration size
//! - Zero-allocation formatting with direct writes to output
//!
//! ## Examples
//!
//! ```
//! use std::time::Duration;
//! use duration_fmt::{human, DurationFormat};
//!
//! // Basic usage
//! let duration = Duration::from_secs(3662); // 1h 1m 2s
//! println!("{}", human(duration)); // Uses default format
//!
//! // With custom format
//! println!("{}", human(duration).format(DurationFormat::Detailed));
//! ```

use core::fmt;
use core::time::Duration;

use rand::Rng as _;

/// Defines the display format for duration formatting.
///
/// Each format option represents a different style of presenting time durations,
/// from compact representations to detailed human-readable formats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DurationFormat {
    /// Automatically selects the most appropriate format based on duration size.
    ///
    /// - For durations with days: uses `Detailed` format
    /// - For durations with hours or minutes: uses `Compact` format
    /// - For durations with seconds: displays seconds with millisecond precision
    /// - For smaller durations: uses appropriate millisecond or microsecond units
    Auto,

    /// Compact format without spaces: `1h2m3s`
    ///
    /// Useful for space-constrained displays or technical outputs.
    Compact,

    /// Standard format with spaces: `1 hour 2 minutes 3 seconds`
    ///
    /// A balanced format for general purpose human-readable output.
    Standard,

    /// Detailed format with commas: `1 hour, 2 minutes, 3 seconds`
    ///
    /// Provides the most formal and complete representation.
    Detailed,

    /// ISO 8601 duration format: `PT1H2M3S`
    ///
    /// Follows the international standard for representing time durations.
    ISO8601,

    /// Fuzzy, human-friendly format: `about 5 minutes`
    ///
    /// Rounds to the most significant unit for casual time indications.
    Fuzzy,

    /// Clock-like numeric format: `01:02:03.456`
    ///
    /// Displays time in a familiar digital clock format.
    Numeric,

    /// Verbose format for debugging: `D:1 H:2 M:3 S:4 MS:567`
    ///
    /// Shows all time components with labels, useful for debugging.
    Verbose,

    /// Randomly selects a format for each display.
    ///
    /// Adds an element of surprise to your time displays!
    Random,
}

// Use macro to define constants
crate::define_typed_constants! {
    u64 => {
        SECONDS_PER_MINUTE = 60,
        SECONDS_PER_HOUR = 3600,
        SECONDS_PER_DAY = 86400,
    }
    u32 => {
        NANOS_PER_MILLI = 1_000_000,
        NANOS_PER_MICRO = 1_000,
    }
}

/// Time unit used in duration formatting.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TimeUnit {
    Day,
    Hour,
    Minute,
    Second,
    Millisecond,
    Microsecond,
}

/// Localization information for a time unit.
struct UnitLocale {
    singular: &'static str,
    plural: &'static str,
    short: &'static str,
    fuzzy_prefix: &'static str,
}

impl UnitLocale {
    #[inline(always)]
    const fn new(
        singular: &'static str,
        plural: &'static str,
        short: &'static str,
        fuzzy_prefix: &'static str,
    ) -> Self {
        Self { singular, plural, short, fuzzy_prefix }
    }
}

// Define English localized strings
crate::define_typed_constants! {
    &'static str => {
        FUZZY_EN = "about ",
        FUZZY_EMPTY = "",
        ABBR_DAY = "d",
        ABBR_HOUR = "h",
        ABBR_MINUTE = "m",
        ABBR_SECOND = "s",
        ABBR_MILLISECOND = "ms",
        ABBR_MICROSECOND = "us",
    }

    UnitLocale => {
        EN_DAY = UnitLocale::new("day", "days", ABBR_DAY, FUZZY_EN),
        EN_HOUR = UnitLocale::new("hour", "hours", ABBR_HOUR, FUZZY_EN),
        EN_MINUTE = UnitLocale::new("minute", "minutes", ABBR_MINUTE, FUZZY_EN),
        EN_SECOND = UnitLocale::new("second", "seconds", ABBR_SECOND, FUZZY_EN),
        EN_MILLISECOND = UnitLocale::new("millisecond", "milliseconds", ABBR_MILLISECOND, FUZZY_EMPTY),
        EN_MICROSECOND = UnitLocale::new("microsecond", "microseconds", ABBR_MICROSECOND, FUZZY_EMPTY),
    }
}

/// A wrapper for Duration that provides human-readable formatting options.
#[must_use = "this HumanDuration does nothing unless displayed or converted to a string"]
pub struct HumanDuration {
    duration: Duration,
    format: DurationFormat,
}

impl HumanDuration {
    #[inline]
    pub const fn new(duration: Duration) -> Self {
        Self { duration, format: DurationFormat::Auto }
    }

    #[must_use = "this returns a new value without modifying the original"]
    #[inline]
    pub const fn format(mut self, format: DurationFormat) -> Self {
        self.format = format;
        self
    }

    #[inline]
    const fn get_parts(&self) -> TimeParts {
        let mut secs = self.duration.as_secs();
        let days = secs / SECONDS_PER_DAY;
        secs %= SECONDS_PER_DAY;
        let hours = (secs / SECONDS_PER_HOUR) as u8;
        secs %= SECONDS_PER_HOUR;
        let minutes = (secs / SECONDS_PER_MINUTE) as u8;
        let seconds = (secs % SECONDS_PER_MINUTE) as u8;

        let nanos = self.duration.subsec_nanos();
        let millis = (nanos / NANOS_PER_MILLI) as u16;
        let micros = ((nanos % NANOS_PER_MILLI) / NANOS_PER_MICRO) as u16;

        TimeParts { days, hours, minutes, seconds, millis, micros, nanos }
    }

    #[inline(always)]
    fn get_unit_locale(&self, unit: TimeUnit) -> &'static UnitLocale {
        match unit {
            TimeUnit::Day => &EN_DAY,
            TimeUnit::Hour => &EN_HOUR,
            TimeUnit::Minute => &EN_MINUTE,
            TimeUnit::Second => &EN_SECOND,
            TimeUnit::Millisecond => &EN_MILLISECOND,
            TimeUnit::Microsecond => &EN_MICROSECOND,
        }
    }

    #[inline(always)]
    fn get_unit_str(&self, unit: TimeUnit, count: u64, short: bool) -> &'static str {
        let locale = self.get_unit_locale(unit);
        if short {
            locale.short
        } else if count > 1 {
            locale.plural
        } else {
            locale.singular
        }
    }

    #[inline]
    fn write_padded(
        f: &mut fmt::Formatter<'_>,
        buf: &mut itoa::Buffer,
        value: u8,
        width: usize,
    ) -> fmt::Result {
        let s = buf.format(value);
        let len = s.len();
        for _ in 0..(width.saturating_sub(len)) {
            f.write_str("0")?;
        }
        f.write_str(s)
    }
}

struct TimeParts {
    days: u64,
    hours: u8,
    minutes: u8,
    seconds: u8,
    millis: u16,
    micros: u16,
    nanos: u32,
}

// Formatting implementation
impl HumanDuration {
    #[inline(never)]
    fn random_format() -> DurationFormat {
        const CANDIDATES: &[DurationFormat] = &[
            DurationFormat::Compact,
            DurationFormat::Standard,
            DurationFormat::Detailed,
            DurationFormat::ISO8601,
            DurationFormat::Fuzzy,
            DurationFormat::Numeric,
            DurationFormat::Verbose,
        ];
        unsafe { *CANDIDATES.get_unchecked(rand::rng().random_range(0..CANDIDATES.len())) }
    }

    fn fmt_compact(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts = self.get_parts();
        let mut buf = itoa::Buffer::new();
        let mut written = false;

        if parts.days > 0 {
            f.write_str(buf.format(parts.days))?;
            f.write_str(self.get_unit_str(TimeUnit::Day, parts.days, true))?;
            written = true;
        }
        if parts.hours > 0 {
            f.write_str(buf.format(parts.hours))?;
            f.write_str(self.get_unit_str(TimeUnit::Hour, parts.hours as u64, true))?;
            written = true;
        }
        if parts.minutes > 0 {
            f.write_str(buf.format(parts.minutes))?;
            f.write_str(self.get_unit_str(TimeUnit::Minute, parts.minutes as u64, true))?;
            written = true;
        }
        if parts.seconds > 0 || !written {
            f.write_str(buf.format(parts.seconds))?;
            f.write_str(self.get_unit_str(TimeUnit::Second, parts.seconds as u64, true))?;
        }
        Ok(())
    }

    fn fmt_standard(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts = self.get_parts();
        let mut buf = itoa::Buffer::new();
        let mut first = true;

        if parts.days > 0 {
            f.write_str(buf.format(parts.days))?;
            f.write_str(" ")?;
            f.write_str(self.get_unit_str(TimeUnit::Day, parts.days, false))?;
            first = false;
        }
        if parts.hours > 0 {
            if !first { f.write_str(" ")?; }
            f.write_str(buf.format(parts.hours))?;
            f.write_str(" ")?;
            f.write_str(self.get_unit_str(TimeUnit::Hour, parts.hours as u64, false))?;
            first = false;
        }
        if parts.minutes > 0 {
            if !first { f.write_str(" ")?; }
            f.write_str(buf.format(parts.minutes))?;
            f.write_str(" ")?;
            f.write_str(self.get_unit_str(TimeUnit::Minute, parts.minutes as u64, false))?;
            first = false;
        }
        if parts.seconds > 0 || first {
            if !first { f.write_str(" ")?; }
            f.write_str(buf.format(parts.seconds))?;
            f.write_str(" ")?;
            f.write_str(self.get_unit_str(TimeUnit::Second, parts.seconds as u64, false))?;
        }
        Ok(())
    }

    fn fmt_detailed(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts = self.get_parts();
        let mut buf = itoa::Buffer::new();
        let separator = ", ";
        let mut first = true;

        if parts.days > 0 {
            f.write_str(buf.format(parts.days))?;
            f.write_str(" ")?;
            f.write_str(self.get_unit_str(TimeUnit::Day, parts.days, false))?;
            first = false;
        }
        if parts.hours > 0 {
            if !first { f.write_str(separator)?; }
            f.write_str(buf.format(parts.hours))?;
            f.write_str(" ")?;
            f.write_str(self.get_unit_str(TimeUnit::Hour, parts.hours as u64, false))?;
            first = false;
        }
        if parts.minutes > 0 {
            if !first { f.write_str(separator)?; }
            f.write_str(buf.format(parts.minutes))?;
            f.write_str(" ")?;
            f.write_str(self.get_unit_str(TimeUnit::Minute, parts.minutes as u64, false))?;
            first = false;
        }

        if !first { f.write_str(separator)?; }
        f.write_str(buf.format(parts.seconds))?;
        f.write_str(".")?;
        Self::write_padded(f, &mut buf, (parts.millis / 100) as u8, 1)?;
        Self::write_padded(f, &mut buf, ((parts.millis / 10) % 10) as u8, 1)?;
        Self::write_padded(f, &mut buf, (parts.millis % 10) as u8, 1)?;
        f.write_str(" ")?;
        f.write_str(self.get_unit_str(TimeUnit::Second, parts.seconds as u64, false))?;
        Ok(())
    }

    fn fmt_iso8601(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts = self.get_parts();
        let mut buf = itoa::Buffer::new();

        f.write_str("P")?;
        if parts.days > 0 {
            f.write_str(buf.format(parts.days))?;
            f.write_str("D")?;
        }
        if parts.hours > 0 || parts.minutes > 0 || parts.seconds > 0 {
            f.write_str("T")?;
            if parts.hours > 0 {
                f.write_str(buf.format(parts.hours))?;
                f.write_str("H")?;
            }
            if parts.minutes > 0 {
                f.write_str(buf.format(parts.minutes))?;
                f.write_str("M")?;
            }
            if parts.seconds > 0 {
                f.write_str(buf.format(parts.seconds))?;
                f.write_str("S")?;
            }
        }
        Ok(())
    }

    fn fmt_fuzzy(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total_secs = self.duration.as_secs();
        let mut buf = itoa::Buffer::new();
        let prefix = self.get_unit_locale(TimeUnit::Second).fuzzy_prefix;

        f.write_str(prefix)?;

        if total_secs < 2 {
            f.write_str("1 ")?;
            f.write_str(self.get_unit_str(TimeUnit::Second, 1, false))?;
        } else if total_secs < 60 {
            f.write_str(buf.format(total_secs))?;
            f.write_str(" ")?;
            f.write_str(self.get_unit_str(TimeUnit::Second, total_secs, false))?;
        } else if total_secs < 120 {
            f.write_str("1 ")?;
            f.write_str(self.get_unit_str(TimeUnit::Minute, 1, false))?;
        } else if total_secs < SECONDS_PER_HOUR {
            let minutes = (total_secs + 30) / 60;
            f.write_str(buf.format(minutes))?;
            f.write_str(" ")?;
            f.write_str(self.get_unit_str(TimeUnit::Minute, minutes, false))?;
        } else if total_secs < SECONDS_PER_HOUR * 2 {
            f.write_str("1 ")?;
            f.write_str(self.get_unit_str(TimeUnit::Hour, 1, false))?;
        } else if total_secs < SECONDS_PER_DAY {
            let hours = (total_secs + 1800) / 3600;
            f.write_str(buf.format(hours))?;
            f.write_str(" ")?;
            f.write_str(self.get_unit_str(TimeUnit::Hour, hours, false))?;
        } else {
            let days = (total_secs + 43200) / 86400;
            f.write_str(buf.format(days))?;
            f.write_str(" ")?;
            f.write_str(self.get_unit_str(TimeUnit::Day, days, false))?;
        }
        Ok(())
    }

    fn fmt_numeric(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts = self.get_parts();
        let mut buf = itoa::Buffer::new();

        if parts.days > 0 {
            f.write_str(buf.format(parts.days))?;
            f.write_str(":")?;
            Self::write_padded(f, &mut buf, parts.hours, 2)?;
        } else {
            Self::write_padded(f, &mut buf, parts.hours, 2)?;
        }

        f.write_str(":")?;
        Self::write_padded(f, &mut buf, parts.minutes, 2)?;
        f.write_str(":")?;
        Self::write_padded(f, &mut buf, parts.seconds, 2)?;
        f.write_str(".")?;
        Self::write_padded(f, &mut buf, (parts.millis / 100) as u8, 1)?;
        Self::write_padded(f, &mut buf, ((parts.millis / 10) % 10) as u8, 1)?;
        Self::write_padded(f, &mut buf, (parts.millis % 10) as u8, 1)?;
        Ok(())
    }

    fn fmt_verbose(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts = self.get_parts();
        let mut buf = itoa::Buffer::new();
        let mut first = true;

        macro_rules! write_component {
            ($val:expr, $label:expr) => {
                if $val > 0 {
                    if !first { f.write_str(" ")?; }
                    f.write_str($label)?;
                    f.write_str(":")?;
                    f.write_str(buf.format($val))?;
                    first = false;
                }
            };
        }

        write_component!(parts.days, "D");
        write_component!(parts.hours, "H");
        write_component!(parts.minutes, "M");
        write_component!(parts.seconds, "S");
        write_component!(parts.millis, "ms");
        write_component!(parts.micros, "us");
        write_component!(parts.nanos, "ns");

        if first { f.write_str("0s")?; }
        Ok(())
    }

    fn fmt_auto(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts = self.get_parts();

        if parts.days > 0 {
            self.fmt_detailed(f)
        } else if parts.hours > 0 || parts.minutes > 0 {
            self.fmt_compact(f)
        } else if parts.seconds > 0 {
            let mut buf = itoa::Buffer::new();
            f.write_str(buf.format(parts.seconds))?;
            f.write_str(".")?;
            Self::write_padded(f, &mut buf, (parts.millis / 100) as u8, 1)?;
            Self::write_padded(f, &mut buf, ((parts.millis / 10) % 10) as u8, 1)?;
            Self::write_padded(f, &mut buf, (parts.millis % 10) as u8, 1)?;
            f.write_str(self.get_unit_str(TimeUnit::Second, parts.seconds as u64, false))
        } else if parts.millis > 0 {
            let mut buf = itoa::Buffer::new();
            f.write_str(buf.format(parts.millis))?;
            f.write_str(self.get_unit_str(TimeUnit::Millisecond, parts.millis as u64, false))
        } else if parts.micros > 0 {
            let mut buf = itoa::Buffer::new();
            f.write_str(buf.format(parts.micros))?;
            f.write_str(self.get_unit_str(TimeUnit::Microsecond, parts.micros as u64, false))
        } else {
            f.write_str("0")?;
            f.write_str(self.get_unit_str(TimeUnit::Second, 0, false))
        }
    }
}

impl fmt::Display for HumanDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.format {
            DurationFormat::Auto => self.fmt_auto(f),
            DurationFormat::Compact => self.fmt_compact(f),
            DurationFormat::Standard => self.fmt_standard(f),
            DurationFormat::Detailed => self.fmt_detailed(f),
            DurationFormat::ISO8601 => self.fmt_iso8601(f),
            DurationFormat::Fuzzy => self.fmt_fuzzy(f),
            DurationFormat::Numeric => self.fmt_numeric(f),
            DurationFormat::Verbose => self.fmt_verbose(f),
            DurationFormat::Random => {
                HumanDuration { duration: self.duration, format: Self::random_format() }.fmt(f)
            }
        }
    }
}

#[inline(always)]
pub const fn human(duration: Duration) -> HumanDuration { HumanDuration::new(duration) }

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use super::*;

    #[test]
    fn test_compact_format() {
        let duration = Duration::from_secs(3662);
        let formatted = human(duration).format(DurationFormat::Compact);
        assert_eq!(formatted.to_string(), "1h1m2s");
    }

    #[test]
    fn test_standard_format() {
        let duration = Duration::from_secs(3662);
        let formatted = human(duration).format(DurationFormat::Standard);
        assert_eq!(formatted.to_string(), "1 hour 1 minute 2 seconds");
    }

    #[test]
    fn test_detailed_format() {
        let duration = Duration::from_secs(3662);
        let formatted = human(duration).format(DurationFormat::Detailed);
        assert_eq!(formatted.to_string(), "1 hour, 1 minute, 2.000 seconds");
    }

    #[test]
    fn test_iso8601_format() {
        let duration = Duration::from_secs(3662);
        let formatted = human(duration).format(DurationFormat::ISO8601);
        assert_eq!(formatted.to_string(), "PT1H1M2S");
    }

    #[test]
    fn test_fuzzy_format() {
        let duration = Duration::from_secs(50);
        let formatted = human(duration).format(DurationFormat::Fuzzy);
        assert_eq!(formatted.to_string(), "about 50 seconds");

        let duration = Duration::from_secs(3600);
        let formatted = human(duration).format(DurationFormat::Fuzzy);
        assert_eq!(formatted.to_string(), "about 1 hour");
    }

    #[test]
    fn test_zero_duration() {
        let duration = Duration::from_secs(0);
        let formatted = human(duration).format(DurationFormat::Compact);
        assert_eq!(formatted.to_string(), "0s");
    }

    #[test]
    fn test_verbose_format() {
        let duration = Duration::from_millis(3662567);
        let formatted = human(duration).format(DurationFormat::Verbose);
        assert_eq!(formatted.to_string(), "H:1 M:1 S:2 ms:567");
    }

    #[test]
    fn test_numeric_format() {
        let duration = Duration::from_millis(3662567);
        let formatted = human(duration).format(DurationFormat::Numeric);
        assert_eq!(formatted.to_string(), "01:01:02.567");
    }

    #[test]
    fn test_numeric_format_with_days() {
        let duration = Duration::from_secs(90061);
        let formatted = human(duration).format(DurationFormat::Numeric);
        assert_eq!(formatted.to_string(), "1:01:01:01.000");
    }
}
