//! Checks exported media duration, streams and complete decoding.
use crate::ffmpeg::FfmpegPaths;
use crate::{
    ffprobe::{ffprobe_args, parse_ffprobe_json, AnalyzeError},
    process_runner::{run, ProcessSpec, RunError, Stream},
};
use std::{ffi::OsString, fmt, path::Path, time::Duration};
use tokio_util::sync::CancellationToken;

/// A failed media verification.
#[derive(Debug)]
pub enum VerifyFailure {
    /// The process runner could not complete a verification command.
    Process(RunError),
    /// FFprobe failed or printed diagnostics.
    ProbeFailed,
    /// FFprobe metadata is invalid or has no media streams.
    Metadata(AnalyzeError),
    /// Duration differs from the requested duration by more than the tolerance.
    Duration {
        expected: Duration,
        actual: Duration,
        tolerance: Duration,
    },
    /// The decode check failed or printed anything on stderr.
    DecodeFailed,
}
impl fmt::Display for VerifyFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(e) => write!(f, "verification process failed: {e}"),
            Self::ProbeFailed => f.write_str("FFprobe could not read the exported media"),
            Self::Metadata(e) => write!(f, "invalid exported media: {e}"),
            Self::Duration { expected, actual, tolerance } => write!(f, "export duration {actual:?} differs from expected {expected:?} (tolerance {tolerance:?})"),
            Self::DecodeFailed => f.write_str("exported media failed the full decode check"),
        }
    }
}
impl std::error::Error for VerifyFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Process(e) => Some(e),
            Self::Metadata(e) => Some(e),
            _ => None,
        }
    }
}

/// Probes and decodes output using caller-resolved tools and cancellation.
pub async fn verify(
    output: &Path,
    expected: Duration,
    tolerance: Duration,
    tools: &FfmpegPaths,
    cancel: CancellationToken,
) -> Result<(), VerifyFailure> {
    let media = probe(output, tools, cancel.clone()).await?;
    if media.duration.abs_diff(expected) > tolerance {
        return Err(VerifyFailure::Duration {
            expected,
            actual: media.duration,
            tolerance,
        });
    }
    let mut args = ["-v", "error", "-nostdin", "-i"]
        .map(OsString::from)
        .to_vec();
    args.push(output.as_os_str().to_owned());
    args.extend(["-f", "null", "-"].map(OsString::from));
    let mut diagnostics = false;
    let result = run(
        ProcessSpec {
            program: tools.ffmpeg.clone(),
            args,
            cwd: None,
        },
        |stream, _| {
            if stream == Stream::Stderr {
                diagnostics = true;
            }
        },
        cancel,
    )
    .await
    .map_err(VerifyFailure::Process)?;
    if result.code != Some(0) || diagnostics {
        return Err(VerifyFailure::DecodeFailed);
    }
    Ok(())
}

pub(crate) struct Probe {
    pub duration: Duration,
    pub frame: Duration,
    pub video: bool,
}

pub(crate) async fn probe(
    output: &Path,
    tools: &FfmpegPaths,
    cancel: CancellationToken,
) -> Result<Probe, VerifyFailure> {
    let args = ffprobe_args(output).map_err(VerifyFailure::Metadata)?;
    let mut json = String::new();
    let mut diagnostics = false;
    let result = run(
        ProcessSpec {
            program: tools.ffprobe.clone(),
            args,
            cwd: None,
        },
        |stream, line| {
            if stream == Stream::Stderr {
                diagnostics = true;
            }
            // Bound metadata memory; unusually large probe output is rejected.
            if stream == Stream::Stdout && json.len() <= 1_048_576 {
                json.push_str(line);
                json.push('\n');
            }
        },
        cancel,
    )
    .await
    .map_err(VerifyFailure::Process)?;
    if result.code != Some(0) || diagnostics || json.len() > 1_048_576 {
        return Err(VerifyFailure::ProbeFailed);
    }
    let info = parse_ffprobe_json(&json).map_err(VerifyFailure::Metadata)?;
    let value: serde_json::Value =
        serde_json::from_str(&json).map_err(|_| VerifyFailure::Metadata(AnalyzeError::NotJson))?;
    let frame = value["streams"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"] != 1)
        .filter_map(|s| s["avg_frame_rate"].as_str())
        .filter_map(|s| s.split_once('/'))
        .filter_map(|(n, d)| Some((n.parse::<f64>().ok()?, d.parse::<f64>().ok()?)))
        .filter_map(|(n, d)| Duration::try_from_secs_f64(d / n).ok())
        .find(|d| !d.is_zero())
        .unwrap_or(Duration::ZERO);
    Ok(Probe {
        duration: Duration::from_millis(info.duration.0),
        frame,
        video: info.video.is_some(),
    })
}

/// Measures the largest keyframe gap, including the trailing partial GOP.
pub(crate) async fn keyframe_interval(
    source: &Path,
    media: &Probe,
    tools: &FfmpegPaths,
    cancel: CancellationToken,
) -> Result<Duration, VerifyFailure> {
    if !media.video {
        return Ok(Duration::from_millis(100));
    }
    let mut args = [
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-skip_frame",
        "nokey",
        "-show_frames",
        "-show_entries",
        "frame=best_effort_timestamp_time",
        "-of",
        "csv=p=0",
    ]
    .map(OsString::from)
    .to_vec();
    args.push(source.as_os_str().to_owned());
    let mut previous = Duration::ZERO;
    let mut interval = media.frame;
    let mut diagnostics = false;
    let result = run(
        ProcessSpec {
            program: tools.ffprobe.clone(),
            args,
            cwd: None,
        },
        |stream, line| {
            if stream == Stream::Stderr {
                diagnostics = true;
            }
            if stream == Stream::Stdout {
                if let Some(time) = line
                    .split(',')
                    .next()
                    .and_then(|s| s.parse::<f64>().ok())
                    .and_then(|s| Duration::try_from_secs_f64(s).ok())
                {
                    interval = interval.max(time.saturating_sub(previous));
                    previous = time;
                }
            }
        },
        cancel,
    )
    .await
    .map_err(VerifyFailure::Process)?;
    if result.code != Some(0) || diagnostics {
        return Err(VerifyFailure::ProbeFailed);
    }
    Ok(interval.max(media.duration.saturating_sub(previous)))
}
