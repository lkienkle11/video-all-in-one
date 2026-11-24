//! Asynchronous local-source analysis through the media process runner.
use crate::{
    ffmpeg::FfmpegPaths,
    ffprobe::{self, ffprobe_args, parse_ffprobe_json},
    process_runner::{run, ProcessSpec, RunError, Stream},
};
use std::{fmt, path::Path};
use tokio_util::sync::CancellationToken;
use vaio_core::domain::media::MediaInfo;

/// Failure to run FFprobe or interpret its bounded JSON output.
#[derive(Debug)]
pub enum AnalyzeError {
    Probe(ffprobe::AnalyzeError),
    Process(RunError),
    Failed,
}

impl fmt::Display for AnalyzeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Probe(error) => write!(f, "{error}"),
            Self::Process(error) => write!(f, "{error}"),
            Self::Failed => f.write_str("FFprobe could not analyze the source"),
        }
    }
}
impl std::error::Error for AnalyzeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Probe(error) => Some(error),
            Self::Process(error) => Some(error),
            Self::Failed => None,
        }
    }
}

/// Analyzes an absolute local path on a caller-owned Tokio runtime.
pub async fn analyze_source(
    path: &Path,
    tools: &FfmpegPaths,
    cancel: CancellationToken,
) -> Result<MediaInfo, AnalyzeError> {
    let args = ffprobe_args(path).map_err(AnalyzeError::Probe)?;
    let mut json = String::new();
    let mut diagnostics = false;
    let exit = run(
        ProcessSpec {
            program: tools.ffprobe.clone(),
            args,
            cwd: None,
        },
        |stream, line| match stream {
            Stream::Stderr => diagnostics = true,
            Stream::Stdout if json.len() <= 1_048_576 => {
                json.push_str(line);
                json.push('\n');
            }
            Stream::Stdout => {}
        },
        cancel,
    )
    .await
    .map_err(AnalyzeError::Process)?;
    if exit.code != Some(0) || diagnostics || json.len() > 1_048_576 {
        return Err(AnalyzeError::Failed);
    }
    parse_ffprobe_json(&json).map_err(AnalyzeError::Probe)
}
