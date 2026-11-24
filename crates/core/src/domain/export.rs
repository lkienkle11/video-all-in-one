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
