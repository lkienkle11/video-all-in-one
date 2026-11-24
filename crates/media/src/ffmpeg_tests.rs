use super::*;
use crate::{ffprobe::parse_ffprobe_json, verification::verify};
use std::{
    fs,
    path::Path,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use vaio_core::{
    domain::{
        export::{CutMode, ExportMode},
        segment::Segment,
        time::TimeMs,
    },
    services::media_planner::{plan_export, ExportRequest},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Fixture {
    pub(super) dir: PathBuf,
    pub(super) source: PathBuf,
    pub(super) tools: FfmpegPaths,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "vaio-export-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let tools = FfmpegPaths {
            ffmpeg: "ffmpeg".into(),
            ffprobe: "ffprobe".into(),
        };
        for tool in [&tools.ffmpeg, &tools.ffprobe] {
            assert!(
                Command::new(tool)
                    .arg("-version")
                    .output()
                    .is_ok_and(|output| output.status.success()),
                "required media tool is missing or not runnable: {}",
                tool.display()
            );
        }
        let source = dir.join("source é 日本.mp4");
        let result = Command::new(&tools.ffmpeg)
            .args([
                "-v",
                "error",
                "-nostdin",
                "-n",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x240:rate=25",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000",
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
                "-c:a",
                "aac",
                "-movflags",
                "+faststart",
            ])
            .arg(&source)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "fixture generation: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        Self { dir, source, tools }
    }
    fn plan(&self, cut: CutMode, mode: ExportMode, ranges: &[(u64, u64)]) -> ExportPlan {
        let info = self.probe(&self.source);
        let segments: Vec<_> = ranges
            .iter()
            .map(|&(a, b)| Segment::new(TimeMs(a), TimeMs(b)))
            .collect();
        plan_export(
            &ExportRequest {
                info: &info,
                source: &self.source,
                segments: &segments,
                cut,
                export: mode,
                out_dir: &self.dir,
                stem: "export",
            },
            &Path::exists,
        )
        .unwrap()
    }
    fn probe(&self, path: &Path) -> vaio_core::domain::media::MediaInfo {
        let output = Command::new(&self.tools.ffprobe)
            .args(crate::ffprobe::ffprobe_args(path).unwrap())
            .output()
            .unwrap();
        assert!(output.status.success());
        parse_ffprobe_json(std::str::from_utf8(&output.stdout).unwrap()).unwrap()
    }
    fn assert_clean(&self, plan: &ExportPlan) {
        for step in &plan.steps {
            assert!(!step.temp_output.exists());
            assert!(!step.final_output.exists());
        }
        for file in &plan.support_files {
            assert!(!file.path.exists());
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[tokio::test]
async fn execute_fast_cut_of_generated_fixture_produces_expected_duration() {
    let f = Fixture::new();
    let plan = f.plan(CutMode::Fast, ExportMode::Separate, &[(1_320, 3_320)]);
    let mut progress = Vec::new();
    let outputs = execute(
        &plan,
        &f.tools,
        |p| progress.push(p),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(outputs, plan.outputs);
    assert!(f.probe(&outputs[0]).duration.0.abs_diff(2_000) <= 1_000);
    assert_eq!(progress.first(), Some(&0.0));
    assert_eq!(progress.last(), Some(&1.0));
    assert!(progress.iter().all(|p| (0.0..=1.0).contains(p)));
    assert!(progress.windows(2).all(|p| p[0] <= p[1]));
    assert!(!plan.steps[0].temp_output.exists());
}

#[tokio::test]
async fn execute_precise_cut_is_within_one_frame_of_requested_boundaries() {
    let f = Fixture::new();
    let plan = f.plan(CutMode::Precise, ExportMode::Separate, &[(1_320, 3_360)]);
    let outputs = execute(&plan, &f.tools, |_| {}, CancellationToken::new())
        .await
        .unwrap();
    assert!(f.probe(&outputs[0]).duration.0.abs_diff(2_040) <= 40);
    // Both starts are frame-aligned at 25 fps. Compare the first decoded frame
    // against the requested source frame; a wrong seek cannot pass on duration alone.
    let frame = |path: &Path, seek: &str| {
        let out = Command::new(&f.tools.ffmpeg)
            .args(["-v", "error", "-ss", seek, "-i"])
            .arg(path)
            .args([
                "-frames:v",
                "1",
                "-pix_fmt",
                "gray",
                "-f",
                "rawvideo",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(out.status.success());
        out.stdout
    };
    let expected = frame(&f.source, "1.320");
    let actual = frame(&outputs[0], "0");
    assert_eq!(actual.len(), 320 * 240);
    assert_eq!(expected.len(), actual.len());
    let error: u64 = expected
        .iter()
        .zip(&actual)
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    assert!(error as f64 / (actual.len() as f64) < 3.0);
}

#[tokio::test]
async fn merged_export_duration_equals_sum_of_segments() {
    let f = Fixture::new();
    let plan = f.plan(
        CutMode::Precise,
        ExportMode::Merged,
        &[(320, 1_360), (3_320, 5_360)],
    );
    let mut progress = Vec::new();
    let outputs = execute(
        &plan,
        &f.tools,
        |p| progress.push(p),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(f.probe(&outputs[0]).duration.0.abs_diff(3_080) <= 140);
    assert!(progress.windows(2).all(|p| p[0] <= p[1]));
    assert!(progress.iter().all(|p| (0.0..=1.0).contains(p)));
    // The first piece is 1040 ms of a 6160 ms total (cuts plus concat).
    assert!(progress
        .iter()
        .any(|&p| (p - 1040.0 / 6160.0).abs() < 0.001));
    assert_eq!(progress.last(), Some(&1.0));
    for step in &plan.steps {
        assert!(!step.temp_output.exists());
    }
    assert!(!plan.support_files[0].path.exists());
}

#[tokio::test]
async fn failed_step_leaves_no_file_under_final_name() {
    let f = Fixture::new();
    // The first step succeeds; a later failure must roll that final name back too.
    let mut plan = f.plan(
        CutMode::Precise,
        ExportMode::Separate,
        &[(0, 1_000), (3_000, 4_000)],
    );
    plan.steps[1]
        .args
        .insert(0, "-nonexistent-vaio-option".into());
    assert!(execute(&plan, &f.tools, |_| {}, CancellationToken::new())
        .await
        .is_err());
    f.assert_clean(&plan);
}

#[cfg(unix)]
#[tokio::test]
async fn read_only_output_dir_gives_readable_error() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let plan = f.plan(CutMode::Precise, ExportMode::Separate, &[(0, 1_000)]);
    fs::set_permissions(&f.dir, fs::Permissions::from_mode(0o555)).unwrap();
    let result = execute(&plan, &f.tools, |_| {}, CancellationToken::new()).await;
    fs::set_permissions(&f.dir, fs::Permissions::from_mode(0o755)).unwrap();
    let text = result.unwrap_err().to_string();
    assert!(text.contains("not writable"), "{text}");
    assert!(text.contains(f.dir.to_str().unwrap()), "{text}");
    f.assert_clean(&plan);
}

#[tokio::test]
async fn cancel_removes_temp_files() {
    let f = Fixture::new();
    let plan = f.plan(
        CutMode::Precise,
        ExportMode::Merged,
        &[(0, 2_000), (3_000, 5_000)],
    );
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let mut cancelled_during_work = false;
    let result = execute(
        &plan,
        &f.tools,
        |p| {
            if p > 0.0 && !cancelled_during_work {
                assert!(plan.support_files[0].path.exists());
                assert!(plan.steps[0].temp_output.exists());
                cancelled_during_work = true;
                trigger.cancel();
            }
        },
        cancel,
    )
    .await;
    assert!(cancelled_during_work);
    assert!(matches!(result, Err(ExecError::Cancelled)), "{result:?}");
    f.assert_clean(&plan);
}

#[tokio::test]
async fn verify_rejects_a_truncated_file() {
    let f = Fixture::new();
    verify(
        &f.source,
        Duration::from_secs(8),
        Duration::from_millis(140),
        &f.tools,
        CancellationToken::new(),
    )
    .await
    .unwrap();
    let truncated = f.dir.join("truncated.mp4");
    let bytes = fs::read(&f.source).unwrap();
    fs::write(&truncated, &bytes[..bytes.len() / 2]).unwrap();
    // faststart leaves valid metadata, so a metadata-only verifier would accept it.
    assert_eq!(f.probe(&truncated).duration, TimeMs(8_000));
    assert!(verify(
        &truncated,
        Duration::from_secs(8),
        Duration::from_millis(140),
        &f.tools,
        CancellationToken::new()
    )
    .await
    .is_err());
}

#[tokio::test]
async fn source_file_is_unchanged_after_export() {
    let f = Fixture::new();
    let before = fs::read(&f.source).unwrap();
    let metadata = fs::metadata(&f.source).unwrap();
    let mut plan = f.plan(
        CutMode::Precise,
        ExportMode::Merged,
        &[(0, 1_000), (3_000, 4_000)],
    );
    // Exercise plans without a progress option as well as planner-produced plans.
    for step in &mut plan.steps {
        step.args.drain(6..8);
    }
    execute(&plan, &f.tools, |_| {}, CancellationToken::new())
        .await
        .unwrap();
    let after = fs::metadata(&f.source).unwrap();
    assert_eq!(metadata.len(), after.len());
    assert_eq!(metadata.modified().unwrap(), after.modified().unwrap());
    assert_eq!(before, fs::read(&f.source).unwrap());
}

#[tokio::test]
async fn final_name_taken_during_execution_is_preserved() {
    let f = Fixture::new();
    let plan = f.plan(CutMode::Precise, ExportMode::Separate, &[(0, 2_000)]);
    let mut created = false;
    let result = execute(
        &plan,
        &f.tools,
        |p| {
            if p > 0.0 && !created {
                fs::write(&plan.outputs[0], b"existing user file").unwrap();
                created = true;
            }
        },
        CancellationToken::new(),
    )
    .await;
    assert!(created);
    assert!(
        matches!(result, Err(ExecError::OutputExists(_))),
        "{result:?}"
    );
    assert_eq!(fs::read(&plan.outputs[0]).unwrap(), b"existing user file");
    assert!(!plan.steps[0].temp_output.exists());
}

#[tokio::test]
async fn preexisting_support_file_is_preserved() {
    let f = Fixture::new();
    let plan = f.plan(CutMode::Fast, ExportMode::Merged, &[(0, 1_000)]);
    fs::write(&plan.support_files[0].path, b"user file").unwrap();
    assert!(execute(&plan, &f.tools, |_| {}, CancellationToken::new())
        .await
        .is_err());
    assert_eq!(fs::read(&plan.support_files[0].path).unwrap(), b"user file");
    assert!(!plan.outputs[0].exists());
}

#[tokio::test]
async fn already_cancelled_leaves_no_files() {
    let f = Fixture::new();
    let plan = f.plan(CutMode::Fast, ExportMode::Merged, &[(0, 1_000)]);
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        execute(&plan, &f.tools, |_| {}, cancel).await,
        Err(ExecError::Cancelled)
    ));
    f.assert_clean(&plan);
}
