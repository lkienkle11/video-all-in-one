use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
use tokio_util::sync::CancellationToken;
use vaio_core::{
    domain::{
        export::{CutMode, ExportMode},
        job::{Job, JobId, JobKind, JobState},
    },
    ports::{export_runner::ExportRunner, JobStore, StoreError, StoredJob},
};
use vaio_media::{
    export_runner::{ExportJobPayload, FfmpegExportRunner},
    ffmpeg::FfmpegPaths,
    ffprobe::{ffprobe_args, parse_ffprobe_json},
};

// Only payload lookup is used by the adapter; persistence/recovery is tested in desktop.
struct Stored(StoredJob);
impl JobStore for Stored {
    fn get(&self, id: JobId) -> Result<StoredJob, StoreError> {
        if id == self.0.job.id() {
            Ok(self.0.clone())
        } else {
            Err(StoreError::NotFound(id))
        }
    }
    fn insert(&self, _: JobKind, _: &str) -> Result<JobId, StoreError> {
        unreachable!()
    }
    fn list(&self) -> Result<Vec<StoredJob>, StoreError> {
        unreachable!()
    }
    fn transition(&self, _: JobId, _: JobState, _: Option<&str>) -> Result<(), StoreError> {
        unreachable!()
    }
    fn recover_interrupted(&self) -> Result<Vec<JobId>, StoreError> {
        unreachable!()
    }
    fn claim_next(&self) -> Result<Option<StoredJob>, StoreError> {
        unreachable!()
    }
    fn list_by_state(&self, _: JobState) -> Result<Vec<StoredJob>, StoreError> {
        unreachable!()
    }
}
fn runner(payload: String) -> (FfmpegExportRunner, Job) {
    let job = Job::new(JobId(17), JobKind::Cut);
    let store = Arc::new(Stored(StoredJob {
        job: job.clone(),
        payload,
        created_at_ms: 0,
        updated_at_ms: 0,
    }));
    (FfmpegExportRunner::new(store, tools()), job)
}
fn tools() -> FfmpegPaths {
    FfmpegPaths {
        ffmpeg: "/opt/homebrew/bin/ffmpeg".into(),
        ffprobe: "/opt/homebrew/bin/ffprobe".into(),
    }
}
struct Fixture {
    dir: PathBuf,
    source: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "vaio-runner-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let source = dir.join("source é 日本.mp4");
        let result = Command::new(tools().ffmpeg)
            .args([
                "-v",
                "error",
                "-nostdin",
                "-n",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x240:rate=25",
                "-t",
                "8",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-g",
                "25",
                "-sc_threshold",
                "0",
            ])
            .arg(&source)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        Self { dir, source }
    }
    fn payload(&self, cut: CutMode, export: ExportMode, stem: &str) -> ExportJobPayload {
        ExportJobPayload {
            source: self.source.clone(),
            segments: vec![(6_000, 9_000), (2_000, 3_000), (1_000, 2_000)],
            cut,
            export,
            out_dir: self.dir.clone(),
            stem: stem.into(),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
fn duration(path: &Path) -> u64 {
    let output = Command::new(tools().ffprobe)
        .args(ffprobe_args(path).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_ffprobe_json(std::str::from_utf8(&output.stdout).unwrap())
        .unwrap()
        .duration
        .0
}

#[tokio::test]
async fn runner_exports_a_generated_fixture_end_to_end() {
    let f = Fixture::new();
    let original = fs::read(&f.source).unwrap();
    for (index, (cut, mode)) in [
        (CutMode::Precise, ExportMode::Merged),
        (CutMode::Fast, ExportMode::Separate),
        (CutMode::Fast, ExportMode::Merged),
        (CutMode::Precise, ExportMode::Separate),
    ]
    .into_iter()
    .enumerate()
    {
        let stem = format!("export-{index}");
        let (runner, job) = runner(f.payload(cut, mode, &stem).to_json().unwrap());
        let mut progress = Vec::new();
        let outputs = runner
            .run(&job, &mut |p| progress.push(p), CancellationToken::new())
            .await
            .unwrap();
        let expected = match mode {
            ExportMode::Merged => vec![f.dir.join(format!("{stem}.mp4"))],
            ExportMode::Separate => vec![
                f.dir.join(format!("{stem}-01.mp4")),
                f.dir.join(format!("{stem}-02.mp4")),
            ],
        };
        assert_eq!(outputs, expected);
        for path in &outputs {
            let expected_ms = if mode == ExportMode::Merged {
                4_000
            } else {
                2_000
            };
            assert!(duration(path).abs_diff(expected_ms) <= 140);
        }
        assert_eq!(progress.last(), Some(&1.0));
        assert!(progress
            .iter()
            .all(|p| p.is_finite() && (0.0..=1.0).contains(p)));
        assert!(progress.windows(2).all(|p| p[0] <= p[1]));
        runner.cleanup_temp(&job).unwrap();
        assert!(outputs.iter().all(|p| p.exists()));
    }
    assert_eq!(fs::read(&f.source).unwrap(), original);
    assert!(fs::read_dir(&f.dir).unwrap().all(|p| {
        let p = p.unwrap().path();
        p == f.source || p.extension().unwrap() == "mp4" && !p.to_string_lossy().contains("partial")
    }));
}

#[tokio::test]
async fn bad_payload_is_a_readable_failure() {
    for payload in ["not JSON", r#"{"segments":[]}"#] {
        let (runner, job) = runner(payload.into());
        let failure = runner
            .run(
                &job,
                &mut |_| panic!("bad payload made progress"),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(failure.0.contains("export job payload"), "{failure}");
    }
}

#[test]
fn cleanup_removes_only_recomputed_temps_and_preserves_source_and_outputs() {
    let f = Fixture::new();
    let original = fs::read(&f.source).unwrap();
    let completed = f.dir.join("completed.mp4");
    fs::write(&completed, b"completed user output").unwrap();
    for mode in [ExportMode::Merged, ExportMode::Separate] {
        let payload = f.payload(CutMode::Precise, mode, "crash");
        let names = match mode {
            ExportMode::Merged => vec![
                "crash.partial.mp4",
                "crash.piece-01.partial.mp4",
                "crash.piece-02.partial.mp4",
                "crash.concat.txt",
            ],
            ExportMode::Separate => vec!["crash-01.partial.mp4", "crash-02.partial.mp4"],
        };
        for name in &names {
            fs::write(f.dir.join(name), b"interrupted").unwrap();
        }
        let unrelated = f.dir.join("other.partial.mp4");
        fs::write(&unrelated, b"another job").unwrap();
        let (runner, job) = runner(payload.to_json().unwrap());
        runner.cleanup_temp(&job).unwrap();
        runner.cleanup_temp(&job).unwrap();
        assert!(names.iter().all(|name| !f.dir.join(name).exists()));
        assert_eq!(fs::read(&unrelated).unwrap(), b"another job");
    }
    assert_eq!(fs::read(&f.source).unwrap(), original);
    assert_eq!(fs::read(completed).unwrap(), b"completed user output");
}
