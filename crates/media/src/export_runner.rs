//! Connects durable export jobs to the FFmpeg pipeline.
use crate::{
    ffmpeg::{self, list_h264_encoders, FfmpegPaths},
    ffprobe::{ffprobe_args, parse_ffprobe_json},
    process_runner::{run, ProcessSpec, Stream},
};
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, RwLock},
};
use tokio_util::sync::CancellationToken;
use vaio_core::{
    domain::{
        export::{pick_h264_encoder, CutMode, ExportMode, ExportPlan, H264Encoder},
        job::{Job, JobId},
        media::MediaInfo,
        segment::{normalize, Segment},
        time::TimeMs,
    },
    ports::{
        export_runner::{ExportRunner, RunFailure, RunFuture},
        JobStore,
    },
    services::media_planner::{plan_export, ExportRequest},
};

#[derive(Serialize, Deserialize)]
#[serde(remote = "CutMode")]
enum CutModeJson {
    Fast,
    Precise,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "ExportMode")]
enum ExportModeJson {
    Separate,
    Merged,
}

/// Serializable inputs retained by the job store across restarts.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportJobPayload {
    /// Absolute source path.
    pub source: PathBuf,
    /// Requested intervals in milliseconds, normalized before planning.
    pub segments: Vec<(u64, u64)>,
    /// Stream copy or precise re-encoding.
    #[serde(with = "CutModeJson")]
    pub cut: CutMode,
    /// Separate outputs or a concatenated output.
    #[serde(with = "ExportModeJson")]
    pub export: ExportMode,
    /// Absolute output directory.
    pub out_dir: PathBuf,
    /// User-selected output name without an extension.
    pub stem: String,
}
impl ExportJobPayload {
    /// Encodes the payload for durable storage.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    /// Decodes stored inputs without panicking on malformed JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Executable paths that the host can replace while the runner is alive.
/// Clones share one value, so a change made through any clone reaches every reader.
#[derive(Clone)]
pub struct ToolPathsCell(Arc<RwLock<Arc<FfmpegPaths>>>);
impl ToolPathsCell {
    pub fn new(paths: FfmpegPaths) -> Self {
        Self(Arc::new(RwLock::new(Arc::new(paths))))
    }
    /// The paths in force now; later changes do not affect the returned value.
    pub fn get(&self) -> Arc<FfmpegPaths> {
        // A poisoned lock still holds a complete value: the guarded data is one Arc.
        self.0.read().unwrap_or_else(|e| e.into_inner()).clone()
    }
    /// Replaces the paths for every reader of this cell.
    pub fn set(&self, paths: FfmpegPaths) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(paths);
    }
}

/// Media adapter backed by stored job inputs and host-resolved executables.
pub struct FfmpegExportRunner {
    store: Arc<dyn JobStore>,
    tools: ToolPathsCell,
    /// The encoder picked for the FFmpeg at the stored path; a new path is probed again.
    encoder: Mutex<Option<(PathBuf, Option<H264Encoder>)>>,
}
impl FfmpegExportRunner {
    /// Uses the same store as the scheduler; executable resolution belongs to the host.
    pub fn new(store: Arc<dyn JobStore>, tools: FfmpegPaths) -> Self {
        Self::with_shared_tools(store, ToolPathsCell::new(tools))
    }
    /// Reads the paths from `tools` on every run, so the host can change them later.
    pub fn with_shared_tools(store: Arc<dyn JobStore>, tools: ToolPathsCell) -> Self {
        Self {
            store,
            tools,
            encoder: Mutex::new(None),
        }
    }

    /// The H.264 encoder of the FFmpeg in use, probed once per FFmpeg path.
    async fn encoder_for(
        &self,
        ffmpeg: &Path,
        cancel: CancellationToken,
    ) -> Result<Option<H264Encoder>, RunFailure> {
        {
            let cached = self.encoder.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((path, encoder)) = cached.as_ref() {
                if path == ffmpeg {
                    return Ok(*encoder);
                }
            }
        }
        let available = list_h264_encoders(ffmpeg, cancel).await.map_err(failure)?;
        let picked = pick_h264_encoder(&available, std::env::consts::OS);
        *self.encoder.lock().unwrap_or_else(|e| e.into_inner()) = Some((ffmpeg.to_owned(), picked));
        Ok(picked)
    }

    fn payload(store: &dyn JobStore, id: JobId) -> Result<ExportJobPayload, RunFailure> {
        let stored = store.get(id).map_err(failure)?;
        ExportJobPayload::from_json(&stored.payload)
            .map_err(|error| RunFailure(format!("invalid export job payload: {error}")))
    }
}
impl ExportRunner for FfmpegExportRunner {
    fn run<'a>(
        &'a self,
        job: &'a Job,
        progress: &'a mut (dyn FnMut(f32) + Send),
        cancel: CancellationToken,
    ) -> RunFuture<'a> {
        Box::pin(async move {
            let tools = self.tools.get();
            let store = self.store.clone();
            let id = job.id();
            let payload = tokio::task::spawn_blocking(move || Self::payload(store.as_ref(), id))
                .await
                .map_err(failure)??;
            let args = ffprobe_args(&payload.source).map_err(failure)?;
            let mut json = String::new();
            let mut diagnostics = false;
            let status = run(
                ProcessSpec {
                    program: tools.ffprobe.clone(),
                    args,
                    cwd: None,
                },
                |stream, line| {
                    if stream == Stream::Stderr {
                        diagnostics = true;
                    }
                    if stream == Stream::Stdout && json.len() <= 1_048_576 {
                        json.push_str(line);
                        json.push('\n');
                    }
                },
                cancel.clone(),
            )
            .await
            .map_err(failure)?;
            if status.code != Some(0) || diagnostics || json.len() > 1_048_576 {
                return Err(RunFailure(
                    "FFprobe could not analyze the export source".into(),
                ));
            }
            let info = parse_ffprobe_json(&json).map_err(failure)?;
            // A fast cut copies streams, so it needs no encoder and does not probe.
            let encoder = if payload.cut == CutMode::Precise {
                self.encoder_for(&tools.ffmpeg, cancel.clone())
                    .await?
                    .ok_or_else(|| RunFailure("this FFmpeg has no H.264 encoder".into()))?
            } else {
                H264Encoder::OpenH264
            };
            let plan = tokio::task::spawn_blocking(move || plan(&payload, &info, false, encoder))
                .await
                .map_err(failure)??;
            ffmpeg::execute(&plan, &tools, progress, cancel)
                .await
                .map_err(failure)
        })
    }
    fn cleanup_temp(&self, job: &Job) -> Result<(), RunFailure> {
        let payload = Self::payload(self.store.as_ref(), job.id())?;
        // The port is synchronous; the scheduler invokes cleanup on a blocking worker.
        let tools = self.tools.get();
        let output = Command::new(&tools.ffprobe)
            .args(ffprobe_args(&payload.source).map_err(failure)?)
            .output()
            .map_err(failure)?;
        if !output.status.success() || !output.stderr.is_empty() {
            return Err(RunFailure(
                "FFprobe could not analyze the export source for cleanup".into(),
            ));
        }
        let json = std::str::from_utf8(&output.stdout).map_err(failure)?;
        let info = parse_ffprobe_json(json).map_err(failure)?;
        // The encoder only changes arguments, never the temp file names cleanup needs.
        let plan = plan(&payload, &info, true, H264Encoder::OpenH264)?;
        for path in plan
            .steps
            .iter()
            .map(|step| &step.temp_output)
            .chain(plan.support_files.iter().map(|file| &file.path))
        {
            if path == &payload.source || plan.outputs.contains(path) {
                continue;
            }
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(RunFailure(format!(
                        "could not remove export temporary file {}: {error}",
                        path.display()
                    )))
                }
            }
        }
        Ok(())
    }
}

fn failure(error: impl std::fmt::Display) -> RunFailure {
    RunFailure(error.to_string())
}

fn plan(
    payload: &ExportJobPayload,
    info: &MediaInfo,
    recovering: bool,
    precise_encoder: H264Encoder,
) -> Result<ExportPlan, RunFailure> {
    let segments: Vec<_> = payload
        .segments
        .iter()
        .map(|&(a, b)| Segment::new(TimeMs(a), TimeMs(b)))
        .collect();
    let segments = normalize(&segments, info.duration).map_err(failure)?;
    let exists = |path: &Path| {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        // Temp files from the interrupted plan must not cause a numbered replan.
        let temporary = name.contains(".partial.") || name.ends_with(".concat.txt");
        (!recovering || !temporary) && path.exists()
    };
    plan_export(
        &ExportRequest {
            info,
            source: &payload.source,
            segments: &segments,
            cut: payload.cut,
            export: payload.export,
            out_dir: &payload.out_dir,
            stem: &payload.stem,
            precise_encoder,
        },
        &exists,
    )
    .map_err(failure)
}
