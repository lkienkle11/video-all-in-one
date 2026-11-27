//! Workspace data observed by the desktop views.
use crate::commands::batch::{valid_rows, BatchInput};
use gpui_kit::{Context, Task};
use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{runtime::Handle, sync::mpsc::UnboundedReceiver};
use vaio_core::{
    domain::{
        export::{CutMode, ExportMode},
        job::{JobId, JobState},
        media::MediaInfo,
    },
    ports::events::JobEvent,
    services::job_scheduler::JobScheduler,
};
use vaio_media::{export_runner::ToolPathsCell, ffmpeg::FfmpegPaths};

/// Where the app looks for FFmpeg and FFprobe after the folder the user chose.
#[derive(Debug, Clone)]
pub struct ToolSearch {
    path_var: Option<OsString>,
}
impl ToolSearch {
    /// Searches the `PATH` of this process.
    pub fn from_env() -> Self {
        Self::with_path(std::env::var_os("PATH"))
    }
    /// Searches an explicit `PATH` value, so tests never depend on the real environment.
    pub fn with_path(path_var: Option<OsString>) -> Self {
        Self { path_var }
    }
    pub fn path_var(&self) -> Option<&std::ffi::OsStr> {
        self.path_var.as_deref()
    }
}

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

/// What the user has typed and chosen for one source: segment rows as text and the export options.
/// A source the user has not configured has no draft.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceDraft {
    /// Start and end text of each row, in order; invalid rows are kept as typed.
    pub rows: Vec<(String, String)>,
    pub cut: CutMode,
    pub mode: ExportMode,
    /// Empty means the default output folder of the source.
    pub folder: String,
    /// Empty means the source's file name without its extension.
    pub stem: String,
}
impl Default for SourceDraft {
    fn default() -> Self {
        Self {
            rows: vec![],
            cut: CutMode::Fast,
            mode: ExportMode::Separate,
            folder: String::new(),
            stem: String::new(),
        }
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
    tools: ToolPathsCell,
    search: Option<ToolSearch>,
    sources: Vec<Source>,
    selected: Option<PathBuf>,
    jobs: Vec<JobRow>,
    receiver: Option<UnboundedReceiver<JobEvent>>,
    event_task: Option<Task<()>>,
    scheduler: Option<Arc<JobScheduler>>,
    drafts: HashMap<PathBuf, SourceDraft>,
}
impl AppState {
    pub fn new(
        runtime: Handle,
        tools: Option<FfmpegPaths>,
        receiver: UnboundedReceiver<JobEvent>,
    ) -> Self {
        Self {
            runtime,
            // An empty executable is unusable on purpose: a missing tool must not block the shell.
            tools: ToolPathsCell::new(tools.unwrap_or(FfmpegPaths {
                ffmpeg: PathBuf::new(),
                ffprobe: PathBuf::new(),
            })),
            search: None,
            sources: vec![],
            selected: None,
            jobs: vec![],
            receiver: Some(receiver),
            event_task: None,
            scheduler: None,
            drafts: HashMap::new(),
        }
    }
    pub fn runtime(&self) -> &Handle {
        &self.runtime
    }
    /// The paths in force, or `None` while FFprobe is unresolved.
    pub fn tools(&self) -> Option<Arc<FfmpegPaths>> {
        let paths = self.tools.get();
        (!paths.ffprobe.as_os_str().is_empty()).then_some(paths)
    }
    /// Replaces the tool paths for every reader, including a runner built from the same cell.
    pub fn set_tools(&self, paths: FfmpegPaths) {
        self.tools.set(paths);
    }
    /// Shares `cell` with the export runner and turns on the missing-tool checks.
    pub fn with_tool_search(mut self, cell: ToolPathsCell, search: ToolSearch) -> Self {
        self.tools = cell;
        self.search = Some(search);
        self
    }
    /// `None` when the missing-tool checks are off.
    pub fn tool_search(&self) -> Option<&ToolSearch> {
        self.search.as_ref()
    }
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }
    pub fn jobs(&self) -> &[JobRow] {
        &self.jobs
    }
    /// Exports that are queued or currently running.
    pub fn running_job_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|job| matches!(job.state(), Some(JobState::Queued | JobState::Running)))
            .count()
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
    pub fn draft(&self, path: &Path) -> Option<&SourceDraft> {
        self.drafts.get(path)
    }
    /// Changes the draft of `path`, creating a default one first. Views that show the draft
    /// notify themselves; this does not notify observers of the state.
    pub fn edit_draft(&mut self, path: &Path, edit: impl FnOnce(&mut SourceDraft)) {
        edit(self.drafts.entry(path.to_owned()).or_default());
    }
    /// Every source in list order with its draft, as the batch command takes them.
    pub fn batch_inputs(&self) -> Vec<BatchInput> {
        self.sources
            .iter()
            .map(|source| BatchInput {
                path: source.path.clone(),
                info: source.info.clone(),
                draft: self.drafts.get(&source.path).cloned(),
            })
            .collect()
    }
    /// Whether Process All has at least one source to queue.
    pub fn batch_ready(&self) -> bool {
        self.sources.iter().any(|source| {
            self.drafts
                .get(&source.path)
                .is_some_and(|draft| !valid_rows(draft, source.info.duration).is_empty())
        })
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
