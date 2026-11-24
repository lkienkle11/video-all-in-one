//! Shared inputs for the planner tests: a media file, segments and a plan runner.

use std::ffi::OsString;
use std::path::Path;

use crate::domain::export::{CutMode, ExportMode, ExportPlan, PlanError};
use crate::domain::media::{AudioStream, MediaInfo, VideoStream};
use crate::domain::segment::Segment;
use crate::domain::time::TimeMs;
use crate::services::media_planner::{plan_export, ExportRequest};

pub(crate) const COMMON: [&str; 8] = [
    "-hide_banner",
    "-nostdin",
    "-n",
    "-loglevel",
    "error",
    "-nostats",
    "-progress",
    "pipe:1",
];

pub(crate) fn seg(start: u64, end: u64) -> Segment {
    Segment::new(TimeMs(start), TimeMs(end))
}

pub(crate) fn full_info() -> MediaInfo {
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

pub(crate) fn never(_: &Path) -> bool {
    false
}

pub(crate) fn with_common(rest: &[&str]) -> Vec<OsString> {
    COMMON
        .iter()
        .chain(rest.iter())
        .map(|s| OsString::from(*s))
        .collect()
}

pub(crate) fn plan_with(
    info: &MediaInfo,
    source: &str,
    segments: &[Segment],
    cut: CutMode,
    export: ExportMode,
    exists: &dyn Fn(&Path) -> bool,
) -> Result<ExportPlan, PlanError> {
    let request = ExportRequest {
        info,
        source: Path::new(source),
        segments,
        cut,
        export,
        out_dir: Path::new("/out"),
        stem: "clip",
    };
    plan_export(&request, exists)
}

pub(crate) fn plan(
    info: &MediaInfo,
    source: &str,
    segments: &[Segment],
    cut: CutMode,
    export: ExportMode,
) -> Result<ExportPlan, PlanError> {
    plan_with(info, source, segments, cut, export, &never)
}

pub(crate) fn plan_stem(stem: &str) -> Result<ExportPlan, PlanError> {
    let info = full_info();
    let segments = [seg(0, 1_000)];
    let request = ExportRequest {
        info: &info,
        source: Path::new("/videos/in.mp4"),
        segments: &segments,
        cut: CutMode::Fast,
        export: ExportMode::Separate,
        out_dir: Path::new("/out"),
        stem,
    };
    plan_export(&request, &never)
}
