use super::job_scheduler::{JobScheduler, Limits, NewJob};
use crate::{
    domain::job::{Job, JobId, JobKind, JobState},
    ports::{
        events::{EventSender, JobEvent},
        export_runner::{ExportRunner, RunFailure},
        JobStore, StoreError, StoredJob,
    },
};
use std::{
    collections::BTreeMap,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

/// A payload the fake store refuses, standing in for a constraint failure mid-batch.
pub const POISON: &str = "poison";

#[derive(Default)]
pub struct MemoryStore(pub Mutex<BTreeMap<i64, StoredJob>>);

impl JobStore for MemoryStore {
    fn insert(&self, kind: JobKind, payload: &str) -> Result<JobId, StoreError> {
        let mut rows = self.0.lock().unwrap();
        let id = JobId(rows.keys().last().copied().unwrap_or(0) + 1);
        rows.insert(
            id.0,
            StoredJob {
                job: Job::new(id, kind),
                payload: payload.into(),
                created_at_ms: 0,
                updated_at_ms: 0,
            },
        );
        Ok(id)
    }
    fn insert_many(&self, items: &[(JobKind, &str)]) -> Result<Vec<JobId>, StoreError> {
        if items.iter().any(|(_, payload)| *payload == POISON) {
            return Err(StoreError::Backend("the batch is poisoned".into()));
        }
        items
            .iter()
            .map(|(kind, payload)| self.insert(*kind, payload))
            .collect()
    }
    fn get(&self, id: JobId) -> Result<StoredJob, StoreError> {
        self.0
            .lock()
            .unwrap()
            .get(&id.0)
            .cloned()
            .ok_or(StoreError::NotFound(id))
    }
    fn list(&self) -> Result<Vec<StoredJob>, StoreError> {
        Ok(self.0.lock().unwrap().values().cloned().collect())
    }
    fn transition(&self, id: JobId, to: JobState, error: Option<&str>) -> Result<(), StoreError> {
        self.0
            .lock()
            .unwrap()
            .get_mut(&id.0)
            .ok_or(StoreError::NotFound(id))?
            .job
            .transition(to, error.map(str::to_owned))
            .map_err(StoreError::InvalidTransition)
    }
    fn recover_interrupted(&self) -> Result<Vec<JobId>, StoreError> {
        let mut rows = self.0.lock().unwrap();
        let mut ids = Vec::new();
        for row in rows
            .values_mut()
            .filter(|row| row.job.state() == JobState::Running)
        {
            row.job
                .transition(JobState::Failed, Some("interrupted".into()))
                .unwrap();
            ids.push(row.job.id());
        }
        Ok(ids)
    }
    fn claim_next(&self) -> Result<Option<StoredJob>, StoreError> {
        let mut rows = self.0.lock().unwrap();
        let Some(row) = rows
            .values_mut()
            .find(|row| row.job.state() == JobState::Queued)
        else {
            return Ok(None);
        };
        row.job.transition(JobState::Running, None).unwrap();
        Ok(Some(row.clone()))
    }
    fn list_by_state(&self, state: JobState) -> Result<Vec<StoredJob>, StoreError> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|row| row.job.state() == state)
            .collect())
    }
}

pub enum Outcome {
    Success,
    Error,
    Panic,
}
pub struct Started {
    pub job: Job,
    pub persisted: StoredJob,
    pub finish: oneshot::Sender<Outcome>,
    pub cancelled: oneshot::Receiver<()>,
}
pub struct Runner {
    pub store: Arc<MemoryStore>,
    pub started: mpsc::UnboundedSender<Started>,
    pub cleaned: Mutex<Vec<JobId>>,
    pub stale: Mutex<Vec<JobId>>,
    pub cleanup_error: Mutex<bool>,
}
impl ExportRunner for Runner {
    fn run<'a>(
        &'a self,
        job: &'a Job,
        progress: &'a mut (dyn FnMut(f32) + Send),
        cancel: CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<PathBuf>, RunFailure>> + Send + 'a>> {
        Box::pin(async move {
            let (finish, mut done) = oneshot::channel();
            let (cancelled, cancellation) = oneshot::channel();
            self.started
                .send(Started {
                    job: job.clone(),
                    persisted: self.store.get(job.id()).unwrap(),
                    finish,
                    cancelled: cancellation,
                })
                .unwrap();
            progress(0.5);
            let outcome = tokio::select! {
                result = &mut done => result.unwrap(),
                _ = cancel.cancelled() => {
                    let _ = cancelled.send(());
                    done.await.unwrap()
                }
            };
            match outcome {
                Outcome::Success => Ok(vec![PathBuf::from("finished.mp4")]),
                Outcome::Error => Err(RunFailure("encoder failed: disk full".into())),
                Outcome::Panic => panic!("encoder panicked"),
            }
        })
    }
    fn cleanup_temp(&self, job: &Job) -> Result<(), RunFailure> {
        assert_eq!(
            self.store.get(job.id()).unwrap().job.state(),
            JobState::Failed
        );
        if *self.cleanup_error.lock().unwrap() {
            return Err(RunFailure("temp file is locked".into()));
        }
        self.stale.lock().unwrap().retain(|id| *id != job.id());
        self.cleaned.lock().unwrap().push(job.id());
        Ok(())
    }
}

pub struct Harness {
    pub store: Arc<MemoryStore>,
    pub runner: Arc<Runner>,
    pub scheduler: JobScheduler,
    pub starts: mpsc::UnboundedReceiver<Started>,
    pub events: mpsc::UnboundedReceiver<JobEvent>,
}
impl Harness {
    pub fn new() -> Self {
        let store = Arc::new(MemoryStore::default());
        let (started, starts) = mpsc::unbounded_channel();
        let runner = Arc::new(Runner {
            store: store.clone(),
            started,
            cleaned: Mutex::default(),
            stale: Mutex::default(),
            cleanup_error: Mutex::default(),
        });
        let (tx, events) = mpsc::unbounded_channel();
        let scheduler = JobScheduler::new(
            store.clone(),
            runner.clone(),
            EventSender(tx),
            Limits::default(),
        );
        Self {
            store,
            runner,
            scheduler,
            starts,
            events,
        }
    }
    pub async fn started(&mut self) -> Started {
        bounded(self.starts.recv()).await.unwrap()
    }
    pub async fn state(&mut self, id: JobId, state: JobState) {
        loop {
            let event = bounded(self.events.recv()).await.unwrap();
            if event == JobEvent::StateChanged(id, state) {
                assert_eq!(self.store.get(id).unwrap().job.state(), state);
                return;
            }
        }
    }
    pub async fn failed(&mut self, id: JobId) -> String {
        loop {
            if let JobEvent::Failed(event_id, message) = bounded(self.events.recv()).await.unwrap()
            {
                if event_id == id {
                    return message;
                }
            }
        }
    }
}
pub fn work(kind: JobKind) -> NewJob {
    NewJob {
        kind,
        payload: "same logical export".into(),
    }
}
pub async fn bounded<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(3), future)
        .await
        .expect("scheduler did not respond")
}
