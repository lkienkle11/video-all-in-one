//! Turns segments and a cut mode into the FFmpeg steps of an export.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::domain::export::{CutMode, ExportMode, ExportPlan, PlanError, PlanStep, SupportFile};
use crate::domain::media::MediaInfo;
use crate::domain::segment::Segment;
use crate::domain::time::TimeMs;
use crate::domain::tool::Tool;

/// Everything the planner needs for one export.
pub struct ExportRequest<'a> {
    /// What the analysis found in the source.
    pub info: &'a MediaInfo,
    /// The absolute path of the source video.
    pub source: &'a Path,
    /// The segments to export, already normalized by the caller.
    pub segments: &'a [Segment],
    /// Stream copy or re-encode.
    pub cut: CutMode,
    /// One file per segment or one merged file.
    pub export: ExportMode,
    /// The absolute folder the outputs are written to.
    pub out_dir: &'a Path,
    /// The file name without extension.
    pub stem: &'a str,
}

/// Plans an export. `exists` tells whether a path is already taken.
pub fn plan_export(
    request: &ExportRequest<'_>,
    exists: &dyn Fn(&Path) -> bool,
) -> Result<ExportPlan, PlanError> {
    validate(request)?;
    let ext = extension_for(request);
    let out_dir = request.out_dir;

    let bases: Vec<String> = match request.export {
        ExportMode::Separate => (1..=request.segments.len())
            .map(|i| format!("{}-{i:02}", request.stem))
            .collect(),
        ExportMode::Merged => vec![request.stem.to_string()],
    };
    let names: Vec<String> = bases
        .iter()
        .map(|base| free_name(out_dir, base, &ext, request.source, exists))
        .collect::<Result<_, _>>()?;

    let mut steps = Vec::new();
    let mut support_files = Vec::new();
    let mut outputs = Vec::new();

    match request.export {
        ExportMode::Separate => {
            for (segment, name) in request.segments.iter().zip(&names) {
                let final_output = out_dir.join(format!("{name}.{ext}"));
                let temp_output = out_dir.join(format!("{name}.partial.{ext}"));
                steps.push(PlanStep {
                    program: Tool::Ffmpeg,
                    args: segment_args(request, segment, &temp_output),
                    temp_output,
                    final_output: final_output.clone(),
                });
                outputs.push(final_output);
            }
        }
        ExportMode::Merged => {
            let mut pieces: Vec<PathBuf> = Vec::new();
            for (i, segment) in request.segments.iter().enumerate() {
                let piece =
                    out_dir.join(format!("{}.piece-{:02}.partial.{ext}", request.stem, i + 1));
                steps.push(PlanStep {
                    program: Tool::Ffmpeg,
                    args: segment_args(request, segment, &piece),
                    temp_output: piece.clone(),
                    final_output: piece.clone(),
                });
                pieces.push(piece);
            }

            let list_path = out_dir.join(format!("{}.concat.txt", request.stem));
            let final_output = out_dir.join(format!("{}.{ext}", names[0]));
            let temp_output = out_dir.join(format!("{}.partial.{ext}", names[0]));
            let mut args = common_args();
            args.extend(["-f", "concat", "-safe", "0", "-i"].map(OsString::from));
            args.push(list_path.clone().into_os_string());
            args.extend(["-c", "copy"].map(OsString::from));
            if ext == "mp4" {
                args.extend(["-movflags", "+faststart"].map(OsString::from));
            }
            args.push(temp_output.clone().into_os_string());
            steps.push(PlanStep {
                program: Tool::Ffmpeg,
                args,
                temp_output,
                final_output: final_output.clone(),
            });

            support_files.push(SupportFile {
                path: list_path,
                contents: concat_list(&pieces),
            });
            outputs.push(final_output);
        }
    }

    Ok(ExportPlan {
        steps,
        support_files,
        outputs,
    })
}

/// Checks paths, stem, segments and their bounds, in that order.
fn validate(request: &ExportRequest<'_>) -> Result<(), PlanError> {
    if !request.source.is_absolute() {
        return Err(PlanError::NotAbsolute(request.source.to_path_buf()));
    }
    if !request.out_dir.is_absolute() {
        return Err(PlanError::NotAbsolute(request.out_dir.to_path_buf()));
    }
    if !valid_stem(request.stem) {
        return Err(PlanError::InvalidStem);
    }
    if request.segments.is_empty() {
        return Err(PlanError::NoSegments);
    }
    for segment in request.segments {
        if segment.start() >= segment.end() {
            return Err(PlanError::SegmentNotIncreasing);
        }
        if segment.end() > request.info.duration {
            return Err(PlanError::SegmentBeyondDuration);
        }
    }
    Ok(())
}

fn valid_stem(stem: &str) -> bool {
    !stem.is_empty()
        && stem.chars().count() <= 100
        && !stem.starts_with('.')
        && !stem
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
}

/// The output extension: the lowercased source extension for a fast cut, `mp4` for a precise one.
fn extension_for(request: &ExportRequest<'_>) -> String {
    match request.cut {
        CutMode::Precise => "mp4".to_string(),
        CutMode::Fast => request
            .source
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| "mkv".to_string()),
    }
}

/// Returns `base` or `base (n)` for the first candidate that is neither taken nor the source.
fn free_name(
    out_dir: &Path,
    base: &str,
    ext: &str,
    source: &Path,
    exists: &dyn Fn(&Path) -> bool,
) -> Result<String, PlanError> {
    if out_dir.join(format!("{base}.{ext}")) == source {
        return Err(PlanError::OutputEqualsSource);
    }
    let mut number = 1;
    loop {
        let name = if number == 1 {
            base.to_string()
        } else {
            format!("{base} ({number})")
        };
        let path = out_dir.join(format!("{name}.{ext}"));
        if path != source && !exists(&path) {
            return Ok(name);
        }
        number += 1;
    }
}

/// The arguments every FFmpeg step starts with.
fn common_args() -> Vec<OsString> {
    [
        "-hide_banner",
        "-nostdin",
        "-n",
        "-loglevel",
        "error",
        "-nostats",
        "-progress",
        "pipe:1",
    ]
    .map(OsString::from)
    .to_vec()
}

/// Writes a time in seconds with exactly three fraction digits.
fn seconds(time: TimeMs) -> OsString {
    OsString::from(format!("{}.{:03}", time.0 / 1_000, time.0 % 1_000))
}

/// The FFmpeg arguments that cut one segment into `output`.
fn segment_args(request: &ExportRequest<'_>, segment: &Segment, output: &Path) -> Vec<OsString> {
    let info = request.info;
    let mut args = common_args();
    args.extend(["-ss"].map(OsString::from));
    args.push(seconds(segment.start()));
    args.extend(["-to"].map(OsString::from));
    args.push(seconds(segment.end()));
    args.extend(["-i"].map(OsString::from));
    args.push(request.source.as_os_str().to_os_string());
    match request.cut {
        CutMode::Fast => {
            args.extend(
                ["-map", "0", "-c", "copy", "-avoid_negative_ts", "make_zero"].map(OsString::from),
            );
        }
        CutMode::Precise => {
            if info.video.is_some() {
                args.extend(["-map", "0:v:0", "-map", "0:a?"].map(OsString::from));
                args.extend(
                    [
                        "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt",
                        "yuv420p",
                    ]
                    .map(OsString::from),
                );
            } else {
                args.extend(["-map", "0:a"].map(OsString::from));
            }
            if !info.audio.is_empty() {
                args.extend(["-c:a", "aac", "-b:a", "192k"].map(OsString::from));
            }
            args.extend(["-movflags", "+faststart"].map(OsString::from));
        }
    }
    args.push(output.as_os_str().to_os_string());
    args
}

/// The concat demuxer list: one `file '<path>'` line per piece, quotes escaped.
fn concat_list(pieces: &[PathBuf]) -> String {
    pieces
        .iter()
        .map(|p| {
            let escaped = p.to_string_lossy().replace('\'', r"'\''");
            format!("file '{escaped}'\n")
        })
        .collect()
}
