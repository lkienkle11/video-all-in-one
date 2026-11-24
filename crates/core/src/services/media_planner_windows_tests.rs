//! The planner rules that matter on Windows, spelled with drive-letter paths.

use std::path::{Path, PathBuf};

use crate::domain::export::{CutMode, ExportMode, ExportPlan, PlanError};
use crate::domain::media::{AudioStream, MediaInfo, VideoStream};
use crate::domain::segment::Segment;
use crate::domain::time::TimeMs;
use crate::services::media_planner::{plan_export, ExportRequest};

fn info() -> MediaInfo {
    MediaInfo {
        duration: TimeMs(120_000),
        container: "mov,mp4,m4a,3gp,3g2,mj2".to_string(),
        video: Some(VideoStream {
            codec: "h264".to_string(),
            width: 1920,
            height: 1080,
        }),
        audio: vec![AudioStream {
            codec: "aac".to_string(),
            channels: 2,
        }],
    }
}

fn plan(
    source: &str,
    out_dir: &str,
    exists: &dyn Fn(&Path) -> bool,
) -> Result<ExportPlan, PlanError> {
    let info = info();
    let segments = [Segment::new(TimeMs(0), TimeMs(1_000))];
    plan_export(
        &ExportRequest {
            info: &info,
            source: Path::new(source),
            segments: &segments,
            cut: CutMode::Fast,
            export: ExportMode::Separate,
            out_dir: Path::new(out_dir),
            stem: "clip",
        },
        exists,
    )
}

#[test]
fn drive_letter_paths_are_planned() {
    let plan = plan(r"C:\videos\in.mp4", r"C:\out", &|_| false).unwrap();
    assert_eq!(plan.outputs, vec![PathBuf::from(r"C:\out\clip-01.mp4")]);
    assert_eq!(
        plan.steps[0].temp_output,
        PathBuf::from(r"C:\out\clip-01.partial.mp4")
    );
}

#[test]
fn a_rooted_path_without_a_drive_is_not_absolute() {
    let result = plan(r"\videos\in.mp4", r"C:\out", &|_| false);
    assert!(
        matches!(result, Err(PlanError::NotAbsolute(_))),
        "{result:?}"
    );
}

#[test]
fn output_equal_to_source_ignoring_case_is_rejected() {
    let result = plan(r"C:\OUT\CLIP-01.MP4", r"C:\out", &|_| false);
    assert_eq!(result, Err(PlanError::OutputEqualsSource));
}

#[test]
fn an_existing_output_gets_a_numbered_name() {
    let taken = |p: &Path| p == Path::new(r"C:\out\clip-01.mp4");
    let plan = plan(r"C:\videos\in.mp4", r"C:\out", &taken).unwrap();
    assert_eq!(plan.outputs, vec![PathBuf::from(r"C:\out\clip-01 (2).mp4")]);
}

#[test]
fn control_characters_in_the_output_folder_are_refused() {
    let result = plan(r"C:\videos\in.mp4", "C:\\out\nx", &|_| false);
    assert!(matches!(result, Err(PlanError::UnsafePath)), "{result:?}");
}
