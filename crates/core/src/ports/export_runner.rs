//! Execution and temporary-output cleanup supplied by a media adapter.
use crate::domain::job::Job;
use std::{future::Future, path::PathBuf, pin::Pin};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq)]
/// An actionable adapter error, with credentials and other secrets removed by the adapter.
pub struct RunFailure(pub String);

impl std::fmt::Display for RunFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for RunFailure {}

/// Object-safe future for completed, verified output paths or a readable failure.
pub type RunFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<PathBuf>, RunFailure>> + Send + 'a>>;

/// Executes stored work without owning job-state policy.
pub trait ExportRunner: Send + Sync {
    /// Returns only after subprocesses and resources have stopped, including on cancellation.
    /// Progress must be a fraction from zero to one; errors must not expose secrets.
    fn run<'a>(
        &'a self,
        job: &'a Job,
        progress: &'a mut (dyn FnMut(f32) + Send),
        cancel: CancellationToken,
    ) -> RunFuture<'a>;
    /// Removes stale temporary files belonging to this job, never sources or completed outputs.
    /// The scheduler calls this synchronous operation on a blocking worker before retrying.
    fn cleanup_temp(&self, job: &Job) -> Result<(), RunFailure>;
}
