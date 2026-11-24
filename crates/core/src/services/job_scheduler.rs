//! Durable queue orchestration; events answer which state, how far, and why a job failed.

use crate::domain::job::{InvalidTransition, Job, JobId, JobKind, JobState};
use crate::ports::events::JobEvent;
use crate::ports::{events::EventSender, export_runner::ExportRunner, JobStore, StoreError};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};
use tokio::{runtime::Handle, sync::Semaphore, task::spawn_blocking};
use tokio_util::sync::CancellationToken;

/// Opaque work saved durably before it becomes eligible to run.
#[derive(Clone)]
pub struct NewJob {
    /// Chooses the download or encode capacity pool.
    pub kind: JobKind,
    /// Adapter-owned work description, preserved across retries.
    pub payload: String,
}

/// Separate capacity pools; the desktop uses the defaults of two downloads and one encode.
pub struct Limits {
    /// Maximum simultaneous download runners.
    pub downloads: usize,
    /// Maximum simultaneous cut and merge runners combined.
    pub encodes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            downloads: 2,
            encodes: 1,
        }
    }
}

/// Durable scheduling on a caller-owned Tokio runtime.
///
/// Call `recover` before submitting work after startup. Use only one scheduler per store.
/// Runners must observe cancellation and return after their subprocesses have stopped.
pub struct JobScheduler {
    inner: Arc<Queue>,
}

struct Queue {
    store: Arc<dyn JobStore>,
    runner: Arc<dyn ExportRunner>,
    events: EventSender,
    runtime: Handle,
    downloads: Arc<Semaphore>,
    encodes: Arc<Semaphore>,
    // Serializes state decisions, token registration, and events with durable writes.
    active: Mutex<HashMap<JobId, CancellationToken>>,
}

impl JobScheduler {
    /// Creates a scheduler inside the Tokio runtime that will own its tasks.
    ///
    /// # Panics
    /// Panics outside a Tokio runtime, or if a limit exceeds `Semaphore::MAX_PERMITS`.
    /// A zero limit suspends that class of work.
    pub fn new(
        store: Arc<dyn JobStore>,
        runner: Arc<dyn ExportRunner>,
        events: EventSender,
        limits: Limits,
    ) -> Self {
        Self {
            inner: Arc::new(Queue {
                store,
                runner,
                events,
                runtime: Handle::current(),
                downloads: Arc::new(Semaphore::new(limits.downloads)),
                encodes: Arc::new(Semaphore::new(limits.encodes)),
                active: Mutex::default(),
            }),
        }
    }

    /// Reconciles interrupted work and schedules persisted queued work on a blocking worker.
    /// Refuses recovery while this scheduler already owns work, to avoid failing live runners.
    pub async fn recover(&self) -> Result<(), StoreError> {
        let queue = self.inner.clone();
        spawn_blocking(move || {
            let mut active = queue.lock()?;
            if !active.is_empty() {
                return Err(StoreError::Backend(
                    "recover must precede scheduling work".into(),
                ));
            }
            for id in queue.store.recover_interrupted()? {
                let job = queue.store.get(id)?.job;
                queue
                    .runner
                    .cleanup_temp(&job)
                    .map_err(|error| StoreError::Backend(error.to_string()))?;
                queue
                    .events
                    .send(JobEvent::StateChanged(id, JobState::Failed));
            }
            for stored in queue.store.list_by_state(JobState::Queued)? {
                queue.schedule(stored.job, &mut active);
            }
            Ok(())
        })
        .await
        .map_err(|error| StoreError::Backend(error.to_string()))?
    }

    /// Persists before scheduling. Identical submissions create two independent jobs.
    ///
    /// This synchronous API performs the insertion on its caller's thread. Hosts must call it
    /// from a blocking worker when storage may wait on disk or SQLite locks, never a UI handler.
    /// All asynchronous scheduler store operations run through `spawn_blocking`.
    pub fn submit(&self, new_job: NewJob) -> Result<JobId, StoreError> {
        let queue = &self.inner;
        let mut active = queue.lock()?;
        let id = queue.store.insert(new_job.kind, &new_job.payload)?;
        queue
            .events
            .send(JobEvent::StateChanged(id, JobState::Queued));
        queue.schedule(Job::new(id, new_job.kind), &mut active);
        Ok(id)
    }

    /// Requests cancellation; Running remains Running until its runner returns.
    /// Rejected commands and storage failures produce `JobEvent::Failed` without a state event.
    pub fn cancel(&self, id: JobId) {
        self.inner.command(id, move |queue| {
            let mut active = queue.lock()?;
            let job = queue.store.get(id)?.job;
            match job.state() {
                JobState::Queued => {
                    queue.store.transition(id, JobState::Cancelled, None)?;
                    if let Some(token) = active.remove(&id) {
                        token.cancel();
                    }
                    queue
                        .events
                        .send(JobEvent::StateChanged(id, JobState::Cancelled));
                    Ok(())
                }
                JobState::Running => {
                    let token = active.get(&id).ok_or_else(|| {
                        StoreError::Backend("running job requires startup recovery".into())
                    })?;
                    token.cancel();
                    Ok(())
                }
                from => Err(StoreError::InvalidTransition(InvalidTransition {
                    from,
                    to: JobState::Cancelled,
                })),
            }
        });
    }

    /// Cleans only runner-owned temporary files, then requeues the same failed logical work.
    /// Cleanup errors leave the original Failed state and payload untouched.
    pub fn retry(&self, id: JobId) {
        self.inner.command(id, move |queue| {
            let mut active = queue.lock()?;
            let mut job = queue.store.get(id)?.job;
            if job.state() != JobState::Failed {
                return Err(StoreError::InvalidTransition(InvalidTransition {
                    from: job.state(),
                    to: JobState::Queued,
                }));
            }
            queue
                .runner
                .cleanup_temp(&job)
                .map_err(|error| StoreError::Backend(error.to_string()))?;
            queue.store.transition(id, JobState::Queued, None)?;
            job.transition(JobState::Queued, None)
                .map_err(StoreError::InvalidTransition)?;
            queue
                .events
                .send(JobEvent::StateChanged(id, JobState::Queued));
            queue.schedule(job, &mut active);
            Ok(())
        });
    }
}

impl Queue {
    fn lock(&self) -> Result<MutexGuard<'_, HashMap<JobId, CancellationToken>>, StoreError> {
        self.active
            .lock()
            .map_err(|_| StoreError::Backend("scheduler state lock poisoned".into()))
    }

    fn command(
        self: &Arc<Self>,
        id: JobId,
        operation: impl FnOnce(Arc<Self>) -> Result<(), StoreError> + Send + 'static,
    ) {
        let queue = self.clone();
        self.runtime.spawn(async move {
            let worker = queue.clone();
            let result = spawn_blocking(move || operation(worker)).await;
            let error = match result {
                Ok(Ok(())) => return,
                Ok(Err(error)) => error.to_string(),
                Err(error) => error.to_string(),
            };
            queue.events.send(JobEvent::Failed(id, error));
        });
    }

    fn schedule(self: &Arc<Self>, job: Job, active: &mut HashMap<JobId, CancellationToken>) {
        let token = CancellationToken::new();
        active.insert(job.id(), token.clone());
        let queue = self.clone();
        self.runtime.spawn(async move {
            let id = job.id();
            if let Err(error) = queue.execute(job, token).await {
                if let Ok(mut active) = queue.lock() {
                    active.remove(&id);
                }
                queue.events.send(JobEvent::Failed(id, error.to_string()));
            }
        });
    }

    async fn execute(
        self: &Arc<Self>,
        job: Job,
        token: CancellationToken,
    ) -> Result<(), StoreError> {
        let semaphore = match job.kind() {
            JobKind::Download => self.downloads.clone(),
            JobKind::Cut | JobKind::Merge => self.encodes.clone(),
        };
        let _permit = tokio::select! {
            _ = token.cancelled() => return Ok(()),
            permit = semaphore.acquire_owned() => permit.map_err(|error| StoreError::Backend(error.to_string()))?,
        };
        let id = job.id();
        let queue = self.clone();
        let check_token = token.clone();
        let running = spawn_blocking(move || {
            let _active = queue.lock()?;
            if check_token.is_cancelled() {
                return Ok(None);
            }
            queue.store.transition(id, JobState::Running, None)?;
            let running = queue.store.get(id)?.job;
            queue
                .events
                .send(JobEvent::StateChanged(id, JobState::Running));
            Ok::<_, StoreError>(Some(running))
        })
        .await
        .map_err(|error| StoreError::Backend(error.to_string()))??;
        let Some(running) = running else {
            return Ok(());
        };

        // Join a separate task so a panic during construction OR polling of run cannot
        // terminate scheduling. Never drop the runner future to implement cancellation.
        let runner = self.runner.clone();
        let events = self.events.clone();
        let run_token = token.clone();
        let outcome = self
            .runtime
            .spawn(async move {
                let mut progress = move |value| events.send(JobEvent::Progress(id, value));
                runner.run(&running, &mut progress, run_token).await
            })
            .await;
        let failure = match outcome {
            Ok(Ok(_outputs)) => None,
            Ok(Err(error)) => Some(error.to_string()),
            Err(error) => Some(format!("runner task failed: {error}")),
        };
        let queue = self.clone();
        spawn_blocking(move || {
            let mut active = queue.lock()?;
            let state = if token.is_cancelled() {
                JobState::Cancelled
            } else if failure.is_some() {
                JobState::Failed
            } else {
                JobState::Succeeded
            };
            queue.store.transition(id, state, failure.as_deref())?;
            active.remove(&id);
            queue.events.send(JobEvent::StateChanged(id, state));
            if state == JobState::Failed {
                if let Some(message) = failure {
                    queue.events.send(JobEvent::Failed(id, message));
                }
            }
            Ok(())
        })
        .await
        .map_err(|error| StoreError::Backend(error.to_string()))?
    }
}
