//! Media timestamps in whole milliseconds, with the text forms used by the UI.

use std::fmt;
use std::str::FromStr;

/// A position or duration in a media file, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimeMs(pub u64);

/// Why a time text was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeParseError {
    /// The text was empty or only spaces.
    Empty,
    /// The text starts with a minus sign.
    Negative,
    /// The text is not one of the accepted forms.
    Malformed,
    /// The value does not fit in milliseconds held by a `u64`.
    OutOfRange,
}

impl fmt::Display for TimeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            TimeParseError::Empty => "time is empty",
            TimeParseError::Negative => "time cannot be negative",
            TimeParseError::Malformed => "time is not in a supported format",
            TimeParseError::OutOfRange => "time is too large",
        };
        f.write_str(text)
    }
}

impl std::error::Error for TimeParseError {}

impl TimeMs {
    /// Converts seconds to milliseconds, rounding to the nearest millisecond.
    ///
    /// Returns `None` for NaN, infinite or negative input, and for values that
    /// do not fit in `u64` milliseconds.
    pub fn from_secs_f64(secs: f64) -> Option<TimeMs> {
        if !secs.is_finite() || secs < 0.0 {
            return None;
        }
        let ms = (secs * 1000.0).round();
        // `u64::MAX as f64` is 2^64, the first value that does not fit.
        if ms >= u64::MAX as f64 {
            return None;
        }
        Some(TimeMs(ms as u64))
    }

    /// The time in seconds.
    pub fn as_secs_f64(&self) -> f64 {
        self.0 as f64 / 1000.0
    }

    /// Parses `HH:MM:SS`, `HH:MM:SS.mmm`, `MM:SS`, `MM:SS.mmm`, `SS` or `SS.mmm`.
    ///
    /// Surrounding spaces are ignored. In the colon forms the minutes and seconds
    /// fields must be below 60; the leading field may be any size.
    pub fn parse(text: &str) -> Result<TimeMs, TimeParseError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(TimeParseError::Empty);
        }
        if text.starts_with('-') {
            return Err(TimeParseError::Negative);
        }
        let parts: Vec<&str> = text.split(':').collect();
        if parts.len() > 3 {
            return Err(TimeParseError::Malformed);
        }
        let last = parts[parts.len() - 1];
        let (seconds_text, fraction) = match last.split_once('.') {
            Some((whole, fraction)) => (whole, Some(fraction)),
            None => (last, None),
        };
        let seconds = parse_digits(seconds_text)?;
        let fraction_ms = match fraction {
            Some(digits) => parse_fraction(digits)?,
            None => 0,
        };

        let total_seconds = if parts.len() == 1 {
            seconds
        } else {
            let minutes = parse_digits(parts[parts.len() - 2])?;
            if minutes >= 60 || seconds >= 60 {
                return Err(TimeParseError::Malformed);
            }
            let mut total = minutes * 60 + seconds;
            if parts.len() == 3 {
                let hours = parse_digits(parts[0])?;
                total = hours
                    .checked_mul(3_600)
                    .and_then(|h| h.checked_add(total))
                    .ok_or(TimeParseError::OutOfRange)?;
            }
            total
        };

        total_seconds
            .checked_mul(1_000)
            .and_then(|ms| ms.checked_add(fraction_ms))
            .map(TimeMs)
            .ok_or(TimeParseError::OutOfRange)
    }
}

/// Parses a non-empty run of ASCII digits. Digits that do not fit in `u64` are out of range.
fn parse_digits(text: &str) -> Result<u64, TimeParseError> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(TimeParseError::Malformed);
    }
    text.parse::<u64>().map_err(|_| TimeParseError::OutOfRange)
}

/// Converts a fraction of a second (1 to 3 digits) to milliseconds: `5` is 500.
fn parse_fraction(digits: &str) -> Result<u64, TimeParseError> {
    if digits.is_empty() || digits.len() > 3 {
        return Err(TimeParseError::Malformed);
    }
    let padded = format!("{digits:0<3}");
    parse_digits(&padded)
}

impl fmt::Display for TimeMs {
    /// Writes `HH:MM:SS.mmm`; hours have at least two digits and may exceed 99.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ms = self.0;
        let hours = ms / 3_600_000;
        let minutes = (ms / 60_000) % 60;
        let seconds = (ms / 1_000) % 60;
        let millis = ms % 1_000;
        write!(f, "{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
    }
}

impl FromStr for TimeMs {
    type Err = TimeParseError;

    fn from_str(text: &str) -> Result<TimeMs, TimeParseError> {
        TimeMs::parse(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn time_parses_all_accepted_formats() {
        let cases = [
            ("01:02:03", 3_723_000),
            ("01:02:03.456", 3_723_456),
            ("1:02:03.4", 3_723_400),
            ("02:03", 123_000),
            ("02:03.5", 123_500),
            ("00:00.05", 50),
            ("90", 90_000),
            ("90.5", 90_500),
            ("90.125", 90_125),
            ("  90.125  ", 90_125),
            ("00:00:00.5", 500),
            ("100:00:00", 360_000_000),
        ];
        for (text, ms) in cases {
            assert_eq!(text.parse::<TimeMs>(), Ok(TimeMs(ms)), "parsing {text:?}");
        }
        assert_eq!(TimeMs::parse("1:02:03.4"), Ok(TimeMs(3_723_400)));
    }

    #[test]
    fn time_rejects_negative_and_garbage() {
        assert_eq!(TimeMs::parse(""), Err(TimeParseError::Empty));
        assert_eq!(TimeMs::parse("   "), Err(TimeParseError::Empty));
        assert_eq!(TimeMs::parse("-1"), Err(TimeParseError::Negative));
        assert_eq!(TimeMs::parse(" -00:00:01 "), Err(TimeParseError::Negative));
        for text in [
            "abc",
            "1:2:3:4",
            "00:00:00:00",
            "1.5:00",
            "01:60",
            "00:60:00",
            "1.",
            ".5",
            "1:",
            ":30",
            "1.2345",
            "1e3",
            "+5",
            "12 3",
            "1.2.3",
        ] {
            assert_eq!(
                TimeMs::parse(text),
                Err(TimeParseError::Malformed),
                "parsing {text:?}"
            );
        }
        assert_eq!(
            TimeMs::parse("99999999999999999999"),
            Err(TimeParseError::OutOfRange)
        );
        assert_eq!(
            TimeMs::parse("18446744073709552"),
            Err(TimeParseError::OutOfRange)
        );
    }

    #[test]
    fn from_secs_f64_rounds_to_nearest_millisecond() {
        assert_eq!(TimeMs::from_secs_f64(2.0004), Some(TimeMs(2_000)));
        assert_eq!(TimeMs::from_secs_f64(2.0006), Some(TimeMs(2_001)));
        assert_eq!(TimeMs::from_secs_f64(0.0), Some(TimeMs(0)));
    }

    #[test]
    fn from_secs_f64_rejects_nan_infinite_negative_and_overflow() {
        assert_eq!(TimeMs::from_secs_f64(f64::NAN), None);
        assert_eq!(TimeMs::from_secs_f64(f64::INFINITY), None);
        assert_eq!(TimeMs::from_secs_f64(-1.0), None);
        assert_eq!(TimeMs::from_secs_f64(1e300), None);
    }

    #[test]
    fn as_secs_f64_is_seconds() {
        assert_eq!(TimeMs(1_500).as_secs_f64(), 1.5);
    }

    #[test]
    fn display_uses_two_digit_fields_and_three_millisecond_digits() {
        assert_eq!(TimeMs(0).to_string(), "00:00:00.000");
        assert_eq!(TimeMs(3_723_456).to_string(), "01:02:03.456");
        assert_eq!(TimeMs(360_000_000).to_string(), "100:00:00.000");
        assert_eq!(TimeMs(360_000_000_000).to_string(), "100000:00:00.000");
    }

    proptest! {
        #[test]
        fn display_roundtrips_for_any_time(ms in 0u64..360_000_000_000) {
            let t = TimeMs(ms);
            prop_assert_eq!(TimeMs::parse(&format!("{t}")), Ok(t));
        }
    }
}
