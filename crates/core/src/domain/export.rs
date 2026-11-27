//! Export plans: the FFmpeg steps and files that turn segments into outputs.

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use crate::domain::tool::Tool;

/// How a cut is made: stream copy on keyframes, or a re-encode for exact boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutMode {
    /// Stream copy. Cuts snap to keyframes.
    Fast,
    /// Re-encode to H.264 and AAC for exact boundaries.
    Precise,
}

/// An H.264 encoder that an FFmpeg build can offer for a precise cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264Encoder {
    /// OpenH264, the encoder in the bundled LGPL FFmpeg.
    OpenH264,
    /// x264, found in a developer's own FFmpeg; never in a release package.
    X264,
    /// Windows Media Foundation.
    MediaFoundation,
    /// Apple VideoToolbox.
    VideoToolbox,
}

impl H264Encoder {
    /// The name FFmpeg uses for this encoder, as in `-c:v <name>`.
    pub fn ffmpeg_name(&self) -> &'static str {
        match self {
            Self::OpenH264 => "libopenh264",
            Self::X264 => "libx264",
            Self::MediaFoundation => "h264_mf",
            Self::VideoToolbox => "h264_videotoolbox",
        }
    }
}

/// Picks the first available encoder in the order `libopenh264`, `libx264`, `h264_mf`
/// (only when `os` is `windows`), `h264_videotoolbox` (only when `os` is `macos`).
pub fn pick_h264_encoder(available: &[String], os: &str) -> Option<H264Encoder> {
    let has = |name: &str| available.iter().any(|a| a == name);
    if has("libopenh264") {
        Some(H264Encoder::OpenH264)
    } else if has("libx264") {
        Some(H264Encoder::X264)
    } else if os == "windows" && has("h264_mf") {
        Some(H264Encoder::MediaFoundation)
    } else if os == "macos" && has("h264_videotoolbox") {
        Some(H264Encoder::VideoToolbox)
    } else {
        None
    }
}

/// How the segments become files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMode {
    /// One output file per segment.
    Separate,
    /// One output file made by joining every segment.
    Merged,
}

/// One external program run, writing `temp_output` and then renamed to `final_output`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanStep {
    /// The program to run.
    pub program: Tool,
    /// The arguments, each one a separate value. Never joined into a string.
    pub args: Vec<OsString>,
    /// The file the program writes.
    pub temp_output: PathBuf,
    /// The file the step's result is renamed to after verification.
    pub final_output: PathBuf,
}

/// A file the executor writes before the steps run, such as the concat list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportFile {
    /// Where the file is written.
    pub path: PathBuf,
    /// The text written to the file.
    pub contents: String,
}

/// Everything needed to produce the outputs of one export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPlan {
    /// The steps, in the order they run.
    pub steps: Vec<PlanStep>,
    /// The files to write before the first step.
    pub support_files: Vec<SupportFile>,
    /// The final files the user gets, in order.
    pub outputs: Vec<PathBuf>,
}

/// Why an export cannot be planned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// A path that must be absolute is not.
    NotAbsolute(PathBuf),
    /// The file name stem is empty, too long, has a forbidden character or starts with a dot.
    InvalidStem,
    /// There are no segments to export.
    NoSegments,
    /// A segment ends after the end of the source.
    SegmentBeyondDuration,
    /// A segment does not start before it ends.
    SegmentNotIncreasing,
    /// An output would be written over the source video.
    OutputEqualsSource,
    /// The source or the output folder contains a control character, such as a newline.
    UnsafePath,
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::NotAbsolute(path) => write!(f, "path is not absolute: {}", path.display()),
            PlanError::InvalidStem => {
                f.write_str("the output name is empty, too long or has a forbidden character")
            }
            PlanError::NoSegments => f.write_str("there are no segments to export"),
            PlanError::SegmentBeyondDuration => {
                f.write_str("a segment ends after the end of the source")
            }
            PlanError::SegmentNotIncreasing => {
                f.write_str("a segment does not start before it ends")
            }
            PlanError::OutputEqualsSource => {
                f.write_str("an output would overwrite the source video")
            }
            PlanError::UnsafePath => {
                f.write_str("a path contains a control character, such as a newline")
            }
        }
    }
}

impl std::error::Error for PlanError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn pick_prefers_openh264_over_every_other() {
        let all = names(&["h264_videotoolbox", "h264_mf", "libx264", "libopenh264"]);
        assert_eq!(
            pick_h264_encoder(&all, "macos"),
            Some(H264Encoder::OpenH264)
        );
        assert_eq!(
            pick_h264_encoder(&all, "windows"),
            Some(H264Encoder::OpenH264)
        );
    }

    #[test]
    fn pick_prefers_x264_over_platform_encoders() {
        let list = names(&["h264_videotoolbox", "h264_mf", "libx264"]);
        assert_eq!(pick_h264_encoder(&list, "macos"), Some(H264Encoder::X264));
        assert_eq!(pick_h264_encoder(&list, "windows"), Some(H264Encoder::X264));
    }

    #[test]
    fn pick_uses_media_foundation_only_on_windows() {
        let list = names(&["h264_mf"]);
        assert_eq!(
            pick_h264_encoder(&list, "windows"),
            Some(H264Encoder::MediaFoundation)
        );
        assert_eq!(pick_h264_encoder(&list, "linux"), None);
        assert_eq!(pick_h264_encoder(&list, "macos"), None);
    }

    #[test]
    fn pick_uses_videotoolbox_only_on_macos() {
        let list = names(&["h264_videotoolbox"]);
        assert_eq!(
            pick_h264_encoder(&list, "macos"),
            Some(H264Encoder::VideoToolbox)
        );
        assert_eq!(pick_h264_encoder(&list, "linux"), None);
        assert_eq!(pick_h264_encoder(&list, "windows"), None);
    }

    #[test]
    fn pick_returns_none_when_no_h264_encoder() {
        assert_eq!(pick_h264_encoder(&[], "linux"), None);
        assert_eq!(pick_h264_encoder(&names(&["aac", "mpeg4"]), "macos"), None);
    }

    #[test]
    fn the_lgpl_bundle_list_never_picks_x264() {
        let bundle = names(&["libopenh264", "h264_mf"]);
        for os in ["macos", "windows", "linux"] {
            assert_eq!(pick_h264_encoder(&bundle, os), Some(H264Encoder::OpenH264));
        }
        assert_eq!(
            pick_h264_encoder(&names(&["h264_mf"]), "windows"),
            Some(H264Encoder::MediaFoundation)
        );
    }

    #[test]
    fn ffmpeg_names_match_the_encoders() {
        assert_eq!(H264Encoder::OpenH264.ffmpeg_name(), "libopenh264");
        assert_eq!(H264Encoder::X264.ffmpeg_name(), "libx264");
        assert_eq!(H264Encoder::MediaFoundation.ffmpeg_name(), "h264_mf");
        assert_eq!(H264Encoder::VideoToolbox.ffmpeg_name(), "h264_videotoolbox");
    }
}
