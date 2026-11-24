//! Nonblocking scheduler notifications.
use crate::domain::job::{JobId, JobState};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, PartialEq)]
/// Notifications correlated by durable job identifier; storage remains authoritative.
pub enum JobEvent {
    /// Sent only after the state was written successfully.
    StateChanged(JobId, JobState),
    /// Runner-reported fractional progress.
    Progress(JobId, f32),
    /// Readable execution, storage, or command failure; not itself a state transition.
    Failed(JobId, String),
}

#[derive(Clone)]
/// An unbounded event sink. The host must drain the receiver while observing jobs.
pub struct EventSender(pub UnboundedSender<JobEvent>);

impl EventSender {
    /// Sends without waiting; disconnecting an observer never stops job execution.
    pub fn send(&self, event: JobEvent) {
        let _ = self.0.send(event);
    }
}
