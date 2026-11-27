//! Executes planned exports through the media process runner.
use crate::{
    process_runner::{run, ProcessSpec, RunError, Stream},
    progress_parser::parse_progress_line,
    verification::{self, VerifyFailure},
};
use std::{
    fmt, io,
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;
use vaio_core::domain::export::ExportPlan;
use vaio_core::domain::tool::Tool;

mod durations;
mod files;

/// Caller-resolved executables; media does not depend on platform resolution.
pub struct FfmpegPaths {
    /// FFmpeg executable.
    pub ffmpeg: PathBuf,
    /// FFprobe executable.
    pub ffprobe: PathBuf,
}

/// An export failure.
#[derive(Debug)]
pub enum ExecError {
    /// The caller cancelled execution.
    Cancelled,
    /// A final name was taken before publication.
    OutputExists(PathBuf),
    /// A filesystem operation failed in the named output folder.
    Io { folder: PathBuf, source: io::Error },
    /// FFmpeg could not start or finish.
    Process(RunError),
    /// FFmpeg exited unsuccessfully; diagnostics are not exposed as user text.
    StepFailed(Option<i32>),
    /// Output metadata or decoding failed validation.
    Verification(VerifyFailure),
    /// The plan is not a supported FFmpeg cut or concat plan.
    InvalidPlan,
}
impl fmt::Display for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("export cancelled"),
            Self::OutputExists(path) => {
                write!(f, "export name is already taken: {}", path.display())
            }
            Self::Io { folder, source } if source.kind() == io::ErrorKind::PermissionDenied => {
                write!(f, "the folder is not writable: {}", folder.display())
            }
            Self::Io { folder, source } => write!(
                f,
                "export file operation failed in {}: {source}",
                folder.display()
            ),
            Self::Process(e) => write!(f, "export process failed: {e}"),
            Self::StepFailed(code) => write!(f, "FFmpeg export failed (exit code {code:?})"),
            Self::Verification(e) => write!(f, "export verification failed: {e}"),
            Self::InvalidPlan => f.write_str("invalid FFmpeg export plan"),
        }
    }
}
impl std::error::Error for ExecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Process(e) => Some(e),
            Self::Verification(e) => Some(e),
            _ => None,
        }
    }
}
impl From<RunError> for ExecError {
    fn from(error: RunError) -> Self {
        match error {
            RunError::Cancelled => Self::Cancelled,
            other => Self::Process(other),
        }
    }
}
impl From<VerifyFailure> for ExecError {
    fn from(error: VerifyFailure) -> Self {
        match error {
            VerifyFailure::Process(RunError::Cancelled) => Self::Cancelled,
            other => Self::Verification(other),
        }
    }
}

fn file_error(path: &Path, error: io::Error) -> ExecError {
    if error.kind() == io::ErrorKind::AlreadyExists {
        return ExecError::OutputExists(path.to_owned());
    }
    ExecError::Io {
        folder: path.parent().unwrap_or(path).to_owned(),
        source: error,
    }
}

fn check_cancel(cancel: &CancellationToken) -> Result<(), ExecError> {
    if cancel.is_cancelled() {
        Err(ExecError::Cancelled)
    } else {
        Ok(())
    }
}

/// The H.264 encoders FFmpeg can name, in the order the picker prefers them.
const H264_ENCODERS: [&str; 4] = ["libopenh264", "libx264", "h264_mf", "h264_videotoolbox"];

/// Names of the known H.264 encoders in `ffmpeg -hide_banner -encoders` output.
pub fn parse_h264_encoders(encoders_output: &str) -> Vec<String> {
    let listed: Vec<&str> = encoders_output
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .collect();
    H264_ENCODERS
        .iter()
        .filter(|name| listed.contains(name))
        .map(|name| name.to_string())
        .collect()
}

/// Asks the FFmpeg at `ffmpeg` which H.264 encoders it has.
pub async fn list_h264_encoders(
    ffmpeg: &Path,
    cancel: CancellationToken,
) -> Result<Vec<String>, ExecError> {
    let mut listing = String::new();
    let status = run(
        ProcessSpec {
            program: ffmpeg.to_owned(),
            args: ["-hide_banner", "-encoders"].map(Into::into).to_vec(),
            cwd: None,
        },
        |stream, line| {
            if stream == Stream::Stdout && listing.len() <= 1_048_576 {
                listing.push_str(line);
                listing.push('\n');
            }
        },
        cancel,
    )
    .await?;
    if status.code != Some(0) {
        return Err(ExecError::StepFailed(status.code));
    }
    Ok(parse_h264_encoders(&listing))
}

/// Executes and verifies each step, then publishes the planned outputs.
pub async fn execute(
    plan: &ExportPlan,
    tools: &FfmpegPaths,
    mut on_progress: impl FnMut(f32),
    cancel: CancellationToken,
) -> Result<Vec<PathBuf>, ExecError> {
    check_cancel(&cancel)?;
    let mut created = files::Created::default();
    // Ownership is recorded only after exclusive file creation or publication.
    // Dropping the future also rolls back every file owned by this execution.
    let result = execute_steps(plan, tools, &mut on_progress, &cancel, &mut created).await;
    match result {
        Ok(()) => {
            created.cleanup(&plan.outputs)?;
            check_cancel(&cancel)?;
            created.disarm();
            on_progress(1.0);
            Ok(plan.outputs.clone())
        }
        Err(error) => {
            created.cleanup(&[])?;
            Err(error)
        }
    }
}

async fn execute_steps(
    plan: &ExportPlan,
    tools: &FfmpegPaths,
    on_progress: &mut impl FnMut(f32),
    cancel: &CancellationToken,
    created: &mut files::Created,
) -> Result<(), ExecError> {
    let durations = durations::expectations(plan, tools, cancel).await?;
    let total: f64 = durations
        .iter()
        .map(|(duration, _)| duration.as_secs_f64())
        .sum();
    if total <= 0.0 {
        return Err(ExecError::InvalidPlan);
    }
    for file in &plan.support_files {
        check_cancel(cancel)?;
        created.write_support(&file.path, &file.contents)?;
    }
    on_progress(0.0);
    let mut completed = 0.0;
    let mut last = 0.0_f32;
    for (step, &(duration, tolerance)) in plan.steps.iter().zip(&durations) {
        check_cancel(cancel)?;
        if step.program != Tool::Ffmpeg {
            return Err(ExecError::InvalidPlan);
        }
        created.reserve(&step.temp_output)?;
        // FFmpeg may overwrite only the empty file we exclusively reserved.
        // The planner's -n would reject that reservation. Final publication is
        // separately exclusive, including a name taken while FFmpeg is running.
        let mut args = step.args.clone();
        for arg in &mut args {
            if arg == "-n" {
                *arg = "-y".into();
            }
        }
        args.insert(0, "-y".into());
        if !args.iter().any(|a| a == "-progress") {
            args.splice(0..0, ["-progress".into(), "pipe:1".into()]);
        }
        let step_secs = duration.as_secs_f64();
        let status = run(
            ProcessSpec {
                program: tools.ffmpeg.clone(),
                args,
                cwd: None,
            },
            |stream, line| {
                if stream == Stream::Stdout {
                    if let Some(update) = parse_progress_line(line) {
                        let time = if update.done {
                            step_secs
                        } else {
                            (update.out_time.0 as f64 / 1_000.0).min(step_secs)
                        };
                        // Reserve 1.0 for the point after verification and cleanup.
                        let fraction = ((completed + time) / total) as f32;
                        last = last.max(fraction.min(1.0 - f32::EPSILON));
                        on_progress(last);
                    }
                }
            },
            cancel.clone(),
        )
        .await?;
        check_cancel(cancel)?;
        if status.code != Some(0) {
            return Err(ExecError::StepFailed(status.code));
        }
        verification::verify(
            &step.temp_output,
            duration,
            tolerance,
            tools,
            cancel.clone(),
        )
        .await?;
        check_cancel(cancel)?;
        if step.temp_output != step.final_output {
            created.publish(&step.temp_output, &step.final_output)?;
        }
        completed += step_secs;
        last = last.max(((completed / total) as f32).min(1.0 - f32::EPSILON));
        on_progress(last);
    }
    check_cancel(cancel)
}

#[cfg(test)]
#[path = "ffmpeg_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "ffmpeg_more_tests.rs"]
mod more_tests;

#[cfg(test)]
#[path = "ffmpeg_encoder_tests.rs"]
mod encoder_tests;
