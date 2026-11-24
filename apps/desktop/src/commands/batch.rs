//! Process All: one export job per configured source, queued all together or not at all.
use crate::{
    commands::{
        export::{
            default_output_dir, default_stem, error_key, plan_export_job, ExportCommandError,
            ExportOptions,
        },
        segment_rows::check_row,
    },
    state::SourceDraft,
};
use std::path::PathBuf;
use vaio_core::{
    domain::{export::PlanError, job::JobId, media::MediaInfo, segment::Segment, time::TimeMs},
    i18n::{tr, tr_fmt, Language, Text},
    services::job_scheduler::JobScheduler,
};

/// One source of the workspace with the draft the user made for it, if any.
#[derive(Debug, Clone)]
pub struct BatchInput {
    pub path: PathBuf,
    pub info: MediaInfo,
    pub draft: Option<SourceDraft>,
}

/// What Process All did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchOutcome {
    /// The queued jobs, in source order.
    pub jobs: Vec<JobId>,
    /// How many sources had no valid segment row and were left out.
    pub skipped: usize,
}

/// Why nothing was queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchError {
    /// The source whose export could not be planned; `None` when the queue refused the batch.
    pub source: Option<PathBuf>,
    pub error: ExportCommandError,
}

/// Plans every configured source first, then persists all jobs together.
/// This does file-system and database work: call it from a blocking worker.
pub fn process_all(
    scheduler: &JobScheduler,
    inputs: &[BatchInput],
) -> Result<BatchOutcome, BatchError> {
    let mut planned = Vec::new();
    let mut skipped = 0;
    // Folders chosen by this batch, so two sources never share a default folder.
    let mut taken: Vec<PathBuf> = Vec::new();
    for input in inputs {
        let named = |error| BatchError {
            source: Some(input.path.clone()),
            error,
        };
        let rows = input
            .draft
            .as_ref()
            .map(|draft| valid_rows(draft, input.info.duration))
            .unwrap_or_default();
        let (Some(draft), false) = (&input.draft, rows.is_empty()) else {
            skipped += 1;
            continue;
        };
        let out_dir = if draft.folder.trim().is_empty() {
            default_output_dir(&input.path, &|candidate| {
                candidate.exists() || taken.iter().any(|dir| dir == candidate)
            })
            .ok_or_else(|| {
                named(ExportCommandError::Plan(PlanError::NotAbsolute(
                    input.path.clone(),
                )))
            })?
        } else {
            PathBuf::from(draft.folder.trim())
        };
        taken.push(out_dir.clone());
        let stem = match draft.stem.trim() {
            "" => default_stem(&input.path),
            text => text.to_owned(),
        };
        let options = ExportOptions::new(draft.cut, draft.mode, out_dir, stem);
        planned.push((
            input.path.clone(),
            plan_export_job(&input.path, &input.info, &rows, &options).map_err(named)?,
        ));
    }
    for (path, export) in &planned {
        export.create_out_dir().map_err(|error| BatchError {
            source: Some(path.clone()),
            error,
        })?;
    }
    let jobs = scheduler
        .submit_many(planned.into_iter().map(|(_, e)| e.into_job()).collect())
        .map_err(|error| BatchError {
            source: None,
            error: ExportCommandError::Submit(error.to_string()),
        })?;
    Ok(BatchOutcome { jobs, skipped })
}

/// The rows of the draft that parse and fit inside the source; the others are left out.
pub fn valid_rows(draft: &SourceDraft, duration: TimeMs) -> Vec<Segment> {
    draft
        .rows
        .iter()
        .filter_map(|(start, end)| check_row(start, end, duration).ok())
        .collect()
}

/// The localized sentence the UI shows after Process All finished.
pub fn outcome_message(language: Language, outcome: &BatchOutcome) -> String {
    let mut parts = Vec::new();
    if !outcome.jobs.is_empty() {
        let queued = outcome.jobs.len().to_string();
        parts.push(tr_fmt(language, Text::ProcessAllQueued, &[&queued]));
    }
    if outcome.skipped > 0 {
        let skipped = outcome.skipped.to_string();
        parts.push(tr_fmt(language, Text::ProcessAllSkipped, &[&skipped]));
    }
    parts.join(" ")
}

/// The localized sentence that names the source whose export blocked the batch.
pub fn failure_message(language: Language, error: &BatchError) -> String {
    let reason = tr(language, error_key(&error.error));
    match &error.source {
        Some(path) => {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            tr_fmt(language, Text::ProcessAllFailed, &[&name, reason])
        }
        None => reason.to_owned(),
    }
}
