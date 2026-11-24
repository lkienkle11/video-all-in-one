//! Workspace data observed by the desktop views.
use gpui_kit::{Context, Task};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{runtime::Handle, sync::mpsc::UnboundedReceiver};
use vaio_core::{
    domain::{
        job::{JobId, JobState},
        media::MediaInfo,
    },
    ports::events::JobEvent,
    services::job_scheduler::JobScheduler,
};
use vaio_media::ffmpeg::FfmpegPaths;

pub struct Source {
    path: PathBuf,
    info: MediaInfo,
}
impl Source {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn info(&self) -> &MediaInfo {
        &self.info
    }
}

pub struct JobRow {
    id: JobId,
    state: Option<JobState>,
    progress: f32,
    error: Option<String>,
}
impl JobRow {
    pub fn id(&self) -> JobId {
        self.id
    }
    pub fn state(&self) -> Option<JobState> {
        self.state
    }
    pub fn progress(&self) -> f32 {
        self.progress
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// Owns source selection, queue projections and the receiver task's lifetime.
pub struct AppState {
    runtime: Handle,
    tools: Option<Arc<FfmpegPaths>>,
    sources: Vec<Source>,
    selected: Option<PathBuf>,
    jobs: Vec<JobRow>,
    receiver: Option<UnboundedReceiver<JobEvent>>,
    event_task: Option<Task<()>>,
    scheduler: Option<Arc<JobScheduler>>,
}
impl AppState {
    pub fn new(
        runtime: Handle,
        tools: Option<FfmpegPaths>,
        receiver: UnboundedReceiver<JobEvent>,
    ) -> Self {
        Self {
            runtime,
            tools: tools.map(Arc::new),
            sources: vec![],
            selected: None,
            jobs: vec![],
            receiver: Some(receiver),
            event_task: None,
            scheduler: None,
        }
    }
    pub fn runtime(&self) -> &Handle {
        &self.runtime
    }
    pub fn tools(&self) -> Option<Arc<FfmpegPaths>> {
        self.tools.clone()
    }
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }
    pub fn jobs(&self) -> &[JobRow] {
        &self.jobs
    }
    pub fn selected(&self) -> Option<&Source> {
        self.sources
            .iter()
            .find(|source| Some(&source.path) == self.selected.as_ref())
    }
    pub fn with_scheduler(mut self, scheduler: Arc<JobScheduler>) -> Self {
        self.scheduler = Some(scheduler);
        self
    }
    pub fn scheduler(&self) -> Option<&Arc<JobScheduler>> {
        self.scheduler.as_ref()
    }

    pub(crate) fn select(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.sources.iter().any(|source| source.path == path) {
            self.selected = Some(path);
            cx.notify();
        }
    }
    pub(crate) fn add_source(&mut self, path: PathBuf, info: MediaInfo, cx: &mut Context<Self>) {
        if let Some(source) = self.sources.iter_mut().find(|source| source.path == path) {
            source.info = info;
        } else {
            self.sources.push(Source {
                path: path.clone(),
                info,
            });
        }
        self.select(path, cx);
    }

    /// Applies only the event's facts; a diagnostic alone cannot invent a state transition.
    pub fn apply_event(&mut self, event: JobEvent) {
        let id = match &event {
            JobEvent::StateChanged(id, _) | JobEvent::Progress(id, _) | JobEvent::Failed(id, _) => {
                *id
            }
        };
        if !self.jobs.iter().any(|row| row.id == id) {
            self.jobs.push(JobRow {
                id,
                state: None,
                progress: 0.,
                error: None,
            });
            self.jobs.sort_by_key(|row| row.id.0);
        }
        let Some(row) = self.jobs.iter_mut().find(|row| row.id == id) else {
            return;
        };
        match event {
            JobEvent::StateChanged(_, state) => {
                row.state = Some(state);
                if state != JobState::Failed {
                    row.error = None;
                }
                if state == JobState::Queued {
                    row.progress = 0.;
                }
                if state == JobState::Succeeded {
                    row.progress = 1.;
                }
            }
            JobEvent::Progress(_, value) => {
                if value.is_finite() {
                    row.progress = value.clamp(0., 1.);
                }
            }
            JobEvent::Failed(_, message) => row.error = Some(message),
        }
    }

    pub(crate) fn observe_jobs(&mut self, cx: &mut Context<Self>) {
        let Some(mut receiver) = self.receiver.take() else {
            return;
        };
        self.event_task = Some(cx.spawn(async move |state, cx| {
            while let Some(event) = receiver.recv().await {
                if state
                    .update(cx, |state, cx| {
                        state.apply_event(event);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }
}
