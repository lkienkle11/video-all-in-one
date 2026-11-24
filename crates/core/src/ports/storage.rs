//! The job store port: how the core saves and reads jobs without knowing the database.

use std::error::Error;
use std::fmt;

use crate::domain::job::{InvalidTransition, Job, JobId, JobKind, JobState};

/// A job as it is stored, with its opaque payload and its timestamps in milliseconds.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredJob {
    pub job: Job,
    /// Text the core wrote for this job. Storage never interprets it.
    pub payload: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// Why a job store call failed.
#[derive(Debug, Clone, PartialEq)]
pub enum StoreError {
    /// No job has this identifier.
    NotFound(JobId),
    /// The requested state change is not allowed from the job's current state.
    InvalidTransition(InvalidTransition),
    /// The backing store failed; the text says why.
    Backend(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::NotFound(id) => write!(f, "no job with id {}", id.0),
            StoreError::InvalidTransition(error) => write!(f, "{error}"),
            StoreError::Backend(message) => write!(f, "job store failed: {message}"),
        }
    }
}

impl Error for StoreError {}

/// Durable storage for jobs. Every call is complete when it returns.
pub trait JobStore: Send + Sync {
    /// Stores a new queued job and returns its identifier.
    fn insert(&self, kind: JobKind, payload: &str) -> Result<JobId, StoreError>;

    /// Stores several new queued jobs in input order, all or none, and returns their identifiers
    /// in the same order. When any item fails nothing is stored. The default refuses, so a store
    /// that cannot make this atomic never pretends to.
    fn insert_many(&self, items: &[(JobKind, &str)]) -> Result<Vec<JobId>, StoreError> {
        let _ = items;
        Err(StoreError::Backend(
            "this job store cannot insert several jobs at once".to_string(),
        ))
    }

    /// Returns one job, or `NotFound`.
    fn get(&self, id: JobId) -> Result<StoredJob, StoreError>;

    /// Returns every job, ordered by identifier ascending.
    fn list(&self) -> Result<Vec<StoredJob>, StoreError>;

    /// Moves a job to `to`. `error` is kept only for `Failed`. An invalid move leaves the job unchanged.
    fn transition(&self, id: JobId, to: JobState, error: Option<&str>) -> Result<(), StoreError>;

    /// Marks every `Running` job `Failed` with the reason `interrupted` and returns their identifiers.
    fn recover_interrupted(&self) -> Result<Vec<JobId>, StoreError>;

    /// Takes the queued job with the lowest identifier and marks it `Running` in one atomic step.
    /// Returns the updated job, or `None` when no job is queued.
    fn claim_next(&self) -> Result<Option<StoredJob>, StoreError>;

    /// Returns every job in `state`, ordered by identifier ascending.
    fn list_by_state(&self, state: JobState) -> Result<Vec<StoredJob>, StoreError>;
}
