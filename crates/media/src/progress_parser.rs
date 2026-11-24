//! Reads individual FFmpeg `-progress pipe:1` records.
use vaio_core::domain::time::TimeMs;

/// One time or completion record; other progress keys are ignored.
#[derive(Debug, PartialEq, Eq)]
pub struct ProgressUpdate {
    /// Output time, in milliseconds (FFmpeg's out_time_ms is microseconds).
    pub out_time: TimeMs,
    /// Whether FFmpeg printed `progress=end`.
    pub done: bool,
}

/// Parses a timestamp or completion record, ignoring malformed input.
pub fn parse_progress_line(line: &str) -> Option<ProgressUpdate> {
    let (key, value) = line.trim().split_once('=')?;
    let out_time = match key {
        "progress" if value == "end" => {
            return Some(ProgressUpdate {
                out_time: TimeMs(0),
                done: true,
            })
        }
        // Despite its name, FFmpeg's legacy out_time_ms field is microseconds.
        "out_time_us" | "out_time_ms" => TimeMs(value.parse::<u64>().ok()? / 1_000),
        "out_time" => {
            let mut parts = value.split(':');
            let hours = parts.next()?.parse::<u64>().ok()?;
            let minutes = parts.next()?.parse::<u64>().ok()?;
            let seconds = parts.next()?.parse::<f64>().ok()?;
            if parts.next().is_some() || minutes >= 60 || !(0.0..60.0).contains(&seconds) {
                return None;
            }
            let base = hours
                .checked_mul(3_600_000)?
                .checked_add(minutes * 60_000)?;
            TimeMs(base.checked_add((seconds * 1_000.0).floor() as u64)?)
        }
        _ => return None,
    };
    Some(ProgressUpdate {
        out_time,
        done: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress_lines() {
        for (line, ms, done) in [
            ("out_time_us=1234567", 1234, false),
            ("out_time_ms=1234567", 1234, false),
            ("out_time=01:02:03.456789", 3_723_456, false),
            ("out_time=00:00:00.000000", 0, false),
            ("progress=end", 0, true),
        ] {
            assert_eq!(
                parse_progress_line(line),
                Some(ProgressUpdate {
                    out_time: TimeMs(ms),
                    done
                })
            );
        }
    }

    #[test]
    fn ignores_garbage_progress_lines() {
        for line in [
            "",
            "frame=25",
            "progress=continue",
            "progress=garbage",
            "out_time_us=-1",
            "out_time_ms=N/A",
            "out_time=NaN",
            "out_time=00:60:00.000",
            "out_time=00:00:60.000",
            "out_time_us=18446744073709551616",
            "out_time_ms=1=2",
        ] {
            assert_eq!(parse_progress_line(line), None, "{line}");
        }
    }
}
