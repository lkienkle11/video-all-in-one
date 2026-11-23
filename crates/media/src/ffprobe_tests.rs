use std::ffi::OsString;
use std::path::{Path, PathBuf};

use vaio_core::domain::media::{AudioStream, VideoStream};
use vaio_core::domain::time::TimeMs;

use super::{ffprobe_args, parse_ffprobe_json, AnalyzeError};

const VIDEO_AUDIO: &str = include_str!("../tests/fixtures/ffprobe-video-audio.json");
const VIDEO_ONLY: &str = include_str!("../tests/fixtures/ffprobe-video-only.json");
const AUDIO_ONLY: &str = include_str!("../tests/fixtures/ffprobe-audio-only.json");
const COVER_ART: &str = include_str!("../tests/fixtures/ffprobe-audio-with-cover-art.json");
const NOT_MEDIA: &str = include_str!("../tests/fixtures/ffprobe-not-media.json");

/// The format duration line of the video-audio fixture (the one followed by `size`).
const FORMAT_DURATION: &str = "\"duration\": \"2.000000\",\n        \"size\"";

fn absolute_path() -> &'static str {
    if cfg!(windows) {
        "C:\\media\\clip.mp4"
    } else {
        "/media/clip.mp4"
    }
}

#[test]
fn video_and_audio_fixture_is_parsed() {
    let info = parse_ffprobe_json(VIDEO_AUDIO).expect("fixture parses");
    assert_eq!(info.container, "mov,mp4,m4a,3gp,3g2,mj2");
    assert_eq!(info.duration, TimeMs(2000));
    assert_eq!(
        info.video,
        Some(VideoStream {
            codec: "h264".to_string(),
            width: 640,
            height: 360,
        })
    );
    assert_eq!(
        info.audio,
        vec![AudioStream {
            codec: "aac".to_string(),
            channels: 1,
        }]
    );
}

#[test]
fn video_only_fixture_has_no_audio_streams() {
    let info = parse_ffprobe_json(VIDEO_ONLY).expect("fixture parses");
    assert_eq!(
        info.video,
        Some(VideoStream {
            codec: "h264".to_string(),
            width: 320,
            height: 240,
        })
    );
    assert!(info.audio.is_empty());
}

#[test]
fn audio_only_fixture_has_no_video() {
    let info = parse_ffprobe_json(AUDIO_ONLY).expect("fixture parses");
    assert_eq!(info.container, "wav");
    assert_eq!(info.video, None);
    assert_eq!(
        info.audio,
        vec![AudioStream {
            codec: "pcm_s16le".to_string(),
            channels: 2,
        }]
    );
}

#[test]
fn cover_art_is_not_a_video_stream() {
    let info = parse_ffprobe_json(COVER_ART).expect("fixture parses");
    assert_eq!(info.video, None);
    assert_eq!(
        info.audio,
        vec![AudioStream {
            codec: "mp3".to_string(),
            channels: 1,
        }]
    );
}

#[test]
fn empty_object_has_no_streams() {
    assert_eq!(parse_ffprobe_json(NOT_MEDIA), Err(AnalyzeError::NoStreams));
}

#[test]
fn not_json_is_rejected() {
    assert_eq!(parse_ffprobe_json("not json"), Err(AnalyzeError::NotJson));
    assert_eq!(parse_ffprobe_json(""), Err(AnalyzeError::NotJson));
}

#[test]
fn json_array_is_rejected() {
    assert_eq!(parse_ffprobe_json("[]"), Err(AnalyzeError::NotJson));
    assert_eq!(
        parse_ffprobe_json("[{\"streams\": []}]"),
        Err(AnalyzeError::NotJson)
    );
}

#[test]
fn missing_format_duration_falls_back_to_stream_duration() {
    let json = VIDEO_AUDIO.replace(FORMAT_DURATION, "\"duration\": \"N/A\",\n        \"size\"");
    assert_ne!(json, VIDEO_AUDIO, "the replacement must change the fixture");
    let info = parse_ffprobe_json(&json).expect("stream durations remain");
    assert_eq!(info.duration, TimeMs(2000));
}

#[test]
fn no_duration_anywhere_is_missing_duration() {
    let json = VIDEO_AUDIO.replace("\"duration\": \"2.000000\",", "");
    assert_eq!(
        parse_ffprobe_json(&json),
        Err(AnalyzeError::MissingDuration)
    );
}

#[test]
fn negative_or_garbage_duration_is_invalid() {
    for text in ["-1.0", "abc", "nan", "inf"] {
        let json = VIDEO_AUDIO.replace(
            FORMAT_DURATION,
            &format!("\"duration\": \"{text}\",\n        \"size\""),
        );
        assert_eq!(
            parse_ffprobe_json(&json),
            Err(AnalyzeError::InvalidDuration),
            "duration {text:?}"
        );
    }
}

#[test]
fn zero_duration_is_valid() {
    let json = VIDEO_AUDIO.replace(
        FORMAT_DURATION,
        "\"duration\": \"0.000000\",\n        \"size\"",
    );
    let info = parse_ffprobe_json(&json).expect("zero is a valid duration");
    assert_eq!(info.duration, TimeMs(0));
}

#[test]
fn ffprobe_args_are_exact() {
    let args = ffprobe_args(Path::new(absolute_path())).expect("absolute path");
    let expected: Vec<OsString> = [
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
        absolute_path(),
    ]
    .iter()
    .map(OsString::from)
    .collect();
    assert_eq!(args, expected);
}

#[test]
fn ffprobe_args_reject_relative_paths() {
    for text in ["clip.mp4", "media/clip.mp4", "-ss.mp4"] {
        assert_eq!(
            ffprobe_args(Path::new(text)),
            Err(AnalyzeError::NotAbsolute(PathBuf::from(text))),
            "path {text:?}"
        );
    }
}

#[test]
fn ffprobe_args_keep_spaces_and_unicode_in_one_argument() {
    let path = if cfg!(windows) {
        "C:\\My Videos\\Phim ảnh 1.mp4"
    } else {
        "/My Videos/Phim ảnh 1.mp4"
    };
    let args = ffprobe_args(Path::new(path)).expect("absolute path");
    assert_eq!(args.len(), 7);
    assert_eq!(args[6], OsString::from(path));
}

#[test]
fn two_video_streams_use_the_first_non_cover_one() {
    let json = r#"{
        "streams": [
            {"codec_type": "video", "codec_name": "png", "width": 200, "height": 200,
             "disposition": {"attached_pic": 1}},
            {"codec_type": "video", "codec_name": "h264", "width": 640, "height": 360,
             "disposition": {"attached_pic": 0}},
            {"codec_type": "video", "codec_name": "hevc", "width": 1920, "height": 1080,
             "disposition": {"attached_pic": 0}}
        ],
        "format": {"format_name": "mp4", "duration": "1.000000"}
    }"#;
    let info = parse_ffprobe_json(json).expect("two video streams parse");
    assert_eq!(
        info.video,
        Some(VideoStream {
            codec: "h264".to_string(),
            width: 640,
            height: 360,
        })
    );
}

#[test]
fn audio_channels_missing_is_zero() {
    let json = r#"{
        "streams": [
            {"codec_type": "audio", "codec_name": "aac"}
        ],
        "format": {"format_name": "mp4", "duration": "1.000000"}
    }"#;
    let info = parse_ffprobe_json(json).expect("audio without channels parses");
    assert_eq!(
        info.audio,
        vec![AudioStream {
            codec: "aac".to_string(),
            channels: 0,
        }]
    );
}
