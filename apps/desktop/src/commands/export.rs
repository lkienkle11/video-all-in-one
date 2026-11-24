//! Validates an export request and hands it to the durable job scheduler.
use std::{
    fmt,
    path::{Path, PathBuf},
};
use vaio_core::{
    domain::{
        export::{CutMode, ExportMode, PlanError},
        job::{JobId, JobKind},
        media::MediaInfo,
        segment::{normalize, Segment, SegmentError},
    },
    i18n::Text,
    services::{
        job_scheduler::{JobScheduler, NewJob},
        media_planner::{plan_export, ExportRequest},
    },
};
use vaio_media::export_runner::ExportJobPayload;

/// How and where the user wants the export written.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    cut: CutMode,
    mode: ExportMode,
    out_dir: PathBuf,
    stem: String,
}
impl ExportOptions {
    pub fn new(cut: CutMode, mode: ExportMode, out_dir: PathBuf, stem: String) -> Self {
        Self {
            cut,
            mode,
            out_dir,
            stem,
        }
    }
}

/// Why an export could not be queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportCommandError {
    /// No video is selected.
    NoSource,
    /// The application has no job scheduler.
    SchedulerUnavailable,
    /// The segments are empty, reversed or outside the source.
    Segments(SegmentError),
    /// The planner rejected the name, folder or segments.
    Plan(PlanError),
    /// The output folder path names an existing file.
    OutputNotDirectory(PathBuf),
    /// The output folder could not be created.
    OutputCreate(String),
    /// The job inputs could not be encoded.
    Payload(String),
    /// The scheduler or its store refused the job.
    Submit(String),
}
impl fmt::Display for ExportCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSource => f.write_str("no video is selected"),
            Self::SchedulerUnavailable => f.write_str("the job scheduler is unavailable"),
            Self::Segments(error) => write!(f, "{error}"),
            Self::Plan(error) => write!(f, "{error}"),
            Self::OutputNotDirectory(path) => {
                write!(f, "output folder is a file: {}", path.display())
            }
            Self::OutputCreate(error) => write!(f, "could not create the output folder: {error}"),
            Self::Payload(error) => write!(f, "could not encode the export job: {error}"),
            Self::Submit(error) => write!(f, "could not queue the export job: {error}"),
        }
    }
}
impl std::error::Error for ExportCommandError {}

/// An export that passed validation and planning but has not been written or queued yet.
pub struct PlannedExport {
    job: NewJob,
    out_dir: PathBuf,
}
impl PlannedExport {
    pub fn into_job(self) -> NewJob {
        self.job
    }
    /// Creates the output folder; call it only once the whole request is known to be valid.
    pub fn create_out_dir(&self) -> Result<(), ExportCommandError> {
        std::fs::create_dir_all(&self.out_dir)
            .map_err(|error| ExportCommandError::OutputCreate(error.to_string()))
    }
}

/// Validates and plans one export and builds its job. It writes nothing and queues nothing.
pub fn plan_export_job(
    source: &Path,
    info: &MediaInfo,
    rows: &[Segment],
    options: &ExportOptions,
) -> Result<PlannedExport, ExportCommandError> {
    let segments = normalize(rows, info.duration).map_err(ExportCommandError::Segments)?;
    plan_export(
        &ExportRequest {
            info,
            source,
            segments: &segments,
            cut: options.cut,
            export: options.mode,
            out_dir: &options.out_dir,
            stem: &options.stem,
        },
        &|path| path.exists(),
    )
    .map_err(ExportCommandError::Plan)?;
    if options.out_dir.exists() && !options.out_dir.is_dir() {
        return Err(ExportCommandError::OutputNotDirectory(
            options.out_dir.clone(),
        ));
    }
    let payload = ExportJobPayload {
        source: source.to_path_buf(),
        segments: segments
            .iter()
            .map(|segment| (segment.start().0, segment.end().0))
            .collect(),
        cut: options.cut,
        export: options.mode,
        out_dir: options.out_dir.clone(),
        stem: options.stem.clone(),
    }
    .to_json()
    .map_err(|error| ExportCommandError::Payload(error.to_string()))?;
    let kind = match options.mode {
        ExportMode::Merged => JobKind::Merge,
        ExportMode::Separate => JobKind::Cut,
    };
    Ok(PlannedExport {
        job: NewJob { kind, payload },
        out_dir: options.out_dir.clone(),
    })
}

/// Queues one export job. This does file-system and database work: call it from a
/// blocking worker, never from a UI handler.
pub fn submit_export(
    scheduler: &JobScheduler,
    source: &Path,
    info: &MediaInfo,
    rows: &[Segment],
    options: &ExportOptions,
) -> Result<JobId, ExportCommandError> {
    let planned = plan_export_job(source, info, rows, options)?;
    planned.create_out_dir()?;
    scheduler
        .submit(planned.into_job())
        .map_err(|error| ExportCommandError::Submit(error.to_string()))
}

/// The user-facing text that explains `error`.
pub fn error_key(error: &ExportCommandError) -> Text {
    match error {
        ExportCommandError::NoSource => Text::ExportErrorNoSource,
        ExportCommandError::SchedulerUnavailable => Text::ExportErrorUnavailable,
        ExportCommandError::Segments(_) => Text::ExportErrorSegments,
        ExportCommandError::Plan(PlanError::InvalidStem) => Text::ExportErrorStem,
        ExportCommandError::Plan(PlanError::NotAbsolute(_)) => Text::ExportErrorFolder,
        ExportCommandError::Plan(PlanError::OutputEqualsSource) => Text::ExportErrorOverwrite,
        ExportCommandError::Plan(_) => Text::ExportErrorSegments,
        ExportCommandError::OutputNotDirectory(_) => Text::ExportErrorFolderIsFile,
        ExportCommandError::OutputCreate(_) => Text::ExportErrorFolderCreate,
        ExportCommandError::Payload(_) | ExportCommandError::Submit(_) => Text::ExportErrorSubmit,
    }
}

/// A new folder next to the source, named `<stem>-export`, numbered when taken.
/// Never the source's own folder; `None` when the source path is not absolute.
pub fn default_output_dir(source: &Path, exists: &dyn Fn(&Path) -> bool) -> Option<PathBuf> {
    if !source.is_absolute() {
        return None;
    }
    let parent = source.parent()?;
    let stem = source.file_stem()?.to_string_lossy();
    (1..)
        .map(|number| match number {
            1 => parent.join(format!("{stem}-export")),
            n => parent.join(format!("{stem}-export ({n})")),
        })
        .find(|candidate| !exists(candidate))
}

/// The default output name: the source file name without its extension.
pub fn default_stem(source: &Path) -> String {
    source
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}
