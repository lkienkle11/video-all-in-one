//! Reading job rows from SQLite and turning their stored text into domain values.

use rusqlite::Row;
use vaio_core::domain::job::{Job, JobId, JobKind, JobState};
use vaio_core::ports::{StoreError, StoredJob};

/// Selects every job column, in the order `JobRow::read` expects.
pub(crate) const SELECT_JOBS: &str =
    "SELECT id, kind, state, error, payload, created_at_ms, updated_at_ms FROM jobs";

/// One row of the jobs table, before its kind and state text are parsed.
pub(crate) struct JobRow {
    pub id: i64,
    pub kind: String,
    pub state: String,
    pub error: Option<String>,
    pub payload: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl JobRow {
    /// Reads a row selected with `SELECT_JOBS`.
    pub(crate) fn read(row: &Row<'_>) -> rusqlite::Result<JobRow> {
        Ok(JobRow {
            id: row.get(0)?,
            kind: row.get(1)?,
            state: row.get(2)?,
            error: row.get(3)?,
            payload: row.get(4)?,
            created_at_ms: row.get(5)?,
            updated_at_ms: row.get(6)?,
        })
    }

    /// Parses the kind and state text. An unknown value is a `Backend` error that names the row.
    pub(crate) fn into_stored(self) -> Result<StoredJob, StoreError> {
        let kind = parse_kind(&self.kind).ok_or_else(|| {
            StoreError::Backend(format!("job {} has unknown kind {:?}", self.id, self.kind))
        })?;
        let state = parse_state(&self.state).ok_or_else(|| {
            StoreError::Backend(format!(
                "job {} has unknown state {:?}",
                self.id, self.state
            ))
        })?;
        Ok(StoredJob {
            job: Job::from_parts(JobId(self.id), kind, state, self.error),
            payload: self.payload,
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
        })
    }
}

/// The stored text for a kind.
pub(crate) fn kind_text(kind: JobKind) -> &'static str {
    match kind {
        JobKind::Cut => "cut",
        JobKind::Merge => "merge",
        JobKind::Download => "download",
    }
}

/// The stored text for a state.
pub(crate) fn state_text(state: JobState) -> &'static str {
    match state {
        JobState::Queued => "queued",
        JobState::Running => "running",
        JobState::Succeeded => "succeeded",
        JobState::Failed => "failed",
        JobState::Cancelled => "cancelled",
    }
}

fn parse_kind(text: &str) -> Option<JobKind> {
    match text {
        "cut" => Some(JobKind::Cut),
        "merge" => Some(JobKind::Merge),
        "download" => Some(JobKind::Download),
        _ => None,
    }
}

fn parse_state(text: &str) -> Option<JobState> {
    match text {
        "queued" => Some(JobState::Queued),
        "running" => Some(JobState::Running),
        "succeeded" => Some(JobState::Succeeded),
        "failed" => Some(JobState::Failed),
        "cancelled" => Some(JobState::Cancelled),
        _ => None,
    }
}
