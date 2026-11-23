//! Builds the FFprobe command arguments and reads the JSON that FFprobe prints.
//!
//! Nothing here starts a process: the caller runs FFprobe with [`ffprobe_args`]
//! and passes its standard output to [`parse_ffprobe_json`].

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::Value;
use vaio_core::domain::media::{AudioStream, MediaInfo, VideoStream};
use vaio_core::domain::time::TimeMs;

#[cfg(test)]
#[path = "ffprobe_tests.rs"]
mod tests;

/// Why FFprobe output could not be read, or why a source cannot be probed.
#[derive(Debug, Clone, PartialEq)]
pub enum AnalyzeError {
    /// The text is not a JSON object.
    NotJson,
    /// The object has neither a video stream nor an audio stream.
    NoStreams,
    /// No duration was found in the format or in the used streams.
    MissingDuration,
    /// A duration text is negative, not a number, infinite or too large.
    InvalidDuration,
    /// The source path is not absolute, so it is refused before FFprobe runs.
    NotAbsolute(PathBuf),
}

impl fmt::Display for AnalyzeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalyzeError::NotJson => f.write_str("FFprobe output is not a JSON object"),
            AnalyzeError::NoStreams => f.write_str("the media file has no video or audio stream"),
            AnalyzeError::MissingDuration => f.write_str("the media file has no duration"),
            AnalyzeError::InvalidDuration => f.write_str("the media file has an invalid duration"),
            AnalyzeError::NotAbsolute(path) => {
                write!(f, "the media path is not absolute: {}", path.display())
            }
        }
    }
}

impl std::error::Error for AnalyzeError {}

/// The FFprobe arguments for reading `source` as JSON. The source is the last
/// argument, as one `OsString`; it must be absolute so it can never be read as an option.
pub fn ffprobe_args(source: &Path) -> Result<Vec<OsString>, AnalyzeError> {
    if !source.is_absolute() {
        return Err(AnalyzeError::NotAbsolute(source.to_path_buf()));
    }
    let mut args: Vec<OsString> = [
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    args.push(source.as_os_str().to_os_string());
    Ok(args)
}

/// Turns the JSON that FFprobe prints into a [`MediaInfo`].
pub fn parse_ffprobe_json(json: &str) -> Result<MediaInfo, AnalyzeError> {
    let root: Value = serde_json::from_str(json).map_err(|_| AnalyzeError::NotJson)?;
    if !root.is_object() {
        return Err(AnalyzeError::NotJson);
    }
    let streams: &[Value] = root
        .get("streams")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let format = root.get("format");

    let container = format
        .and_then(|f| f.get("format_name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let video_stream = streams.iter().find(|s| is_video_stream(s));
    let video = video_stream.map(|s| VideoStream {
        codec: text_field(s, "codec_name").to_string(),
        width: u32_field(s, "width"),
        height: u32_field(s, "height"),
    });
    let audio_streams: Vec<&Value> = streams
        .iter()
        .filter(|s| text_field(s, "codec_type") == "audio")
        .collect();
    let audio: Vec<AudioStream> = audio_streams
        .iter()
        .map(|s| AudioStream {
            codec: text_field(s, "codec_name").to_string(),
            channels: u32_field(s, "channels"),
        })
        .collect();

    if video.is_none() && audio.is_empty() {
        return Err(AnalyzeError::NoStreams);
    }

    let mut used: Vec<&Value> = audio_streams;
    used.extend(video_stream);
    let duration = duration_of(format, &used)?;

    Ok(MediaInfo {
        duration,
        container,
        video,
        audio,
    })
}

/// True for a video stream that is not cover art (`disposition.attached_pic` is 1).
fn is_video_stream(stream: &Value) -> bool {
    text_field(stream, "codec_type") == "video" && !is_cover_art(stream)
}

fn is_cover_art(stream: &Value) -> bool {
    stream
        .get("disposition")
        .and_then(|d| d.get("attached_pic"))
        .and_then(Value::as_i64)
        == Some(1)
}

/// A string field, or the empty string when it is missing or not a string.
fn text_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

/// A non-negative integer field that fits in `u32`, or 0 when it does not.
fn u32_field(value: &Value, key: &str) -> u32 {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0)
}

/// The format duration, or when it is missing or `N/A`, the longest duration of the used streams.
fn duration_of(format: Option<&Value>, used: &[&Value]) -> Result<TimeMs, AnalyzeError> {
    let format_text = format
        .and_then(|f| f.get("duration"))
        .and_then(Value::as_str)
        .filter(|text| *text != "N/A");
    if let Some(text) = format_text {
        return parse_seconds(text);
    }
    used.iter()
        .filter_map(|s| s.get("duration").and_then(Value::as_str))
        .map(parse_seconds)
        .collect::<Result<Vec<TimeMs>, AnalyzeError>>()?
        .into_iter()
        .max()
        .ok_or(AnalyzeError::MissingDuration)
}

/// Parses a decimal number of seconds into milliseconds.
fn parse_seconds(text: &str) -> Result<TimeMs, AnalyzeError> {
    let secs: f64 = text
        .trim()
        .parse()
        .map_err(|_| AnalyzeError::InvalidDuration)?;
    TimeMs::from_secs_f64(secs).ok_or(AnalyzeError::InvalidDuration)
}
