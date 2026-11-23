//! Job identity, kinds, lifecycle states and the transition rules between them.

use std::error::Error;
use std::fmt;

/// Identifier of a job in the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub i64);

/// What a job does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobKind {
    Cut,
    Merge,
    Download,
}

/// Where a job is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

/// Whether a job may move from `from` to `to`.
///
/// Allowed: Queued to Running or Cancelled, Running to Succeeded, Failed or
/// Cancelled, and Failed back to Queued for a retry. Nothing else is allowed.
pub fn can_transition(from: JobState, to: JobState) -> bool {
    matches!(
        (from, to),
        (JobState::Queued, JobState::Running)
            | (JobState::Queued, JobState::Cancelled)
            | (JobState::Running, JobState::Succeeded)
            | (JobState::Running, JobState::Failed)
            | (JobState::Running, JobState::Cancelled)
            | (JobState::Failed, JobState::Queued)
    )
}

/// A rejected transition, carrying the state the job was in and the one requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTransition {
    pub from: JobState,
    pub to: JobState,
}

impl fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot move a job from {:?} to {:?}", self.from, self.to)
    }
}

impl Error for InvalidTransition {}

/// A job in the queue. Fields are private so every state change goes through `transition`.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    id: JobId,
    kind: JobKind,
    state: JobState,
    error: Option<String>,
}

impl Job {
    /// A new job, queued and without an error.
    pub fn new(id: JobId, kind: JobKind) -> Job {
        Job {
            id,
            kind,
            state: JobState::Queued,
            error: None,
        }
    }

    pub fn id(&self) -> JobId {
        self.id
    }

    pub fn kind(&self) -> JobKind {
        self.kind
    }

    pub fn state(&self) -> JobState {
        self.state
    }

    /// The error text, set only while the job is `Failed`.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Moves the job to `to` when `can_transition` allows it; otherwise leaves the job unchanged.
    ///
    /// A transition to `Failed` keeps `error`. Every other transition clears it, so a
    /// retry to `Queued` has no error.
    pub fn transition(
        &mut self,
        to: JobState,
        error: Option<String>,
    ) -> Result<(), InvalidTransition> {
        if !can_transition(self.state, to) {
            return Err(InvalidTransition {
                from: self.state,
                to,
            });
        }
        self.state = to;
        self.error = if to == JobState::Failed { error } else { None };
        Ok(())
    }
}

/// After a restart, a job still `Running` was interrupted: mark it `Failed` with the reason
/// `"interrupted"`. Jobs in any other state are left unchanged.
pub fn reconcile_after_restart(jobs: &mut [Job]) {
    for job in jobs.iter_mut().filter(|job| job.state == JobState::Running) {
        // can_transition(Running, Failed) holds, so this is the same change `transition` would make.
        job.state = JobState::Failed;
        job.error = Some("interrupted".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::JobState::*;
    use super::*;
    use proptest::prelude::*;

    const ALL_STATES: [JobState; 5] = [Queued, Running, Succeeded, Failed, Cancelled];

    const ALLOWED: [(JobState, JobState); 6] = [
        (Queued, Running),
        (Queued, Cancelled),
        (Running, Succeeded),
        (Running, Failed),
        (Running, Cancelled),
        (Failed, Queued),
    ];

    /// A job brought to `state` through legal transitions only.
    fn job_in(state: JobState) -> Job {
        let steps: &[JobState] = match state {
            Queued => &[],
            Running => &[Running],
            Succeeded => &[Running, Succeeded],
            Failed => &[Running, Failed],
            Cancelled => &[Cancelled],
        };
        let mut job = Job::new(JobId(1), JobKind::Cut);
        for step in steps {
            job.transition(*step, None).unwrap();
        }
        assert_eq!(job.state(), state);
        job
    }

    #[test]
    fn new_job_is_queued_without_error() {
        let job = Job::new(JobId(7), JobKind::Merge);
        assert_eq!(job.id(), JobId(7));
        assert_eq!(job.kind(), JobKind::Merge);
        assert_eq!(job.state(), Queued);
        assert_eq!(job.error(), None);
    }

    #[test]
    fn succeeded_is_terminal() {
        for to in ALL_STATES {
            assert!(!can_transition(Succeeded, to), "Succeeded -> {to:?}");
        }
        let mut job = job_in(Succeeded);
        let before = job.clone();
        assert_eq!(
            job.transition(Queued, None),
            Err(InvalidTransition {
                from: Succeeded,
                to: Queued
            })
        );
        assert_eq!(job, before);
    }

    #[test]
    fn cancelled_is_terminal() {
        for to in ALL_STATES {
            assert!(!can_transition(Cancelled, to), "Cancelled -> {to:?}");
        }
        let mut job = job_in(Cancelled);
        let before = job.clone();
        assert_eq!(
            job.transition(Running, None),
            Err(InvalidTransition {
                from: Cancelled,
                to: Running
            })
        );
        assert_eq!(job, before);
    }

    #[test]
    fn failed_transition_stores_the_error_text() {
        let mut job = job_in(Running);
        job.transition(Failed, Some("disk full".to_string()))
            .unwrap();
        assert_eq!(job.state(), Failed);
        assert_eq!(job.error(), Some("disk full"));
    }

    #[test]
    fn retry_clears_the_error() {
        let mut job = job_in(Running);
        job.transition(Failed, Some("disk full".to_string()))
            .unwrap();
        job.transition(Queued, None).unwrap();
        assert_eq!(job.state(), Queued);
        assert_eq!(job.error(), None);
    }

    #[test]
    fn other_transitions_keep_error_none() {
        let mut job = Job::new(JobId(1), JobKind::Download);
        job.transition(Running, Some("ignored".to_string()))
            .unwrap();
        assert_eq!(job.error(), None);
        job.transition(Succeeded, Some("ignored".to_string()))
            .unwrap();
        assert_eq!(job.error(), None);
    }

    #[test]
    fn invalid_transition_leaves_job_unchanged() {
        let mut job = Job::new(JobId(1), JobKind::Cut);
        let before = job.clone();
        assert_eq!(
            job.transition(Succeeded, None),
            Err(InvalidTransition {
                from: Queued,
                to: Succeeded
            })
        );
        assert_eq!(job, before);
    }

    #[test]
    fn reconcile_marks_running_jobs_failed_and_leaves_others() {
        let mut jobs = vec![
            job_in(Running),
            job_in(Queued),
            job_in(Succeeded),
            job_in(Cancelled),
            job_in(Failed),
        ];
        let before = jobs.clone();
        reconcile_after_restart(&mut jobs);
        assert_eq!(jobs[0].state(), Failed);
        assert_eq!(jobs[0].error(), Some("interrupted"));
        assert_eq!(jobs[1..], before[1..]);
    }

    proptest! {
        #[test]
        fn only_listed_transitions_are_allowed(
            from in prop::sample::select(ALL_STATES.to_vec()),
            to in prop::sample::select(ALL_STATES.to_vec()),
        ) {
            prop_assert_eq!(can_transition(from, to), ALLOWED.contains(&(from, to)));
        }
    }
}
