//! Turns the start and end text of one segment row into a validated segment.
use vaio_core::domain::{segment::Segment, time::TimeMs};

/// Why a row cannot be exported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowError {
    Invalid,
    NotIncreasing,
    BeyondDuration,
}

/// Parses one row against the source duration.
pub fn check_row(start: &str, end: &str, duration: TimeMs) -> Result<Segment, RowError> {
    let (Ok(start), Ok(end)) = (TimeMs::parse(start), TimeMs::parse(end)) else {
        return Err(RowError::Invalid);
    };
    if start >= end {
        return Err(RowError::NotIncreasing);
    }
    if end > duration {
        return Err(RowError::BeyondDuration);
    }
    Ok(Segment::new(start, end))
}
