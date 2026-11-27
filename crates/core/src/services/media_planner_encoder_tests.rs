//! Tests for the encoder-specific arguments of a precise cut.

use std::ffi::OsString;
use std::path::Path;

use crate::domain::export::{CutMode, ExportMode, H264Encoder};
use crate::services::media_planner::{plan_export, video_bitrate_bps, ExportRequest};
use crate::services::media_planner_fixtures::{full_info, never, seg};

fn precise_args(encoder: H264Encoder, width: u32, height: u32) -> Vec<OsString> {
    let mut info = full_info();
    info.video.as_mut().unwrap().width = width;
    info.video.as_mut().unwrap().height = height;
    info.audio.clear();
    let segments = [seg(0, 5_000)];
    let request = ExportRequest {
        info: &info,
        source: Path::new("/videos/in.mp4"),
        segments: &segments,
        cut: CutMode::Precise,
        export: ExportMode::Separate,
        out_dir: Path::new("/out"),
        stem: "clip",
        precise_encoder: encoder,
    };
    plan_export(&request, &never).unwrap().steps[0].args.clone()
}

fn video_args(args: &[OsString]) -> Vec<String> {
    let at = args.iter().position(|a| a == "-c:v").unwrap();
    let end = args.iter().position(|a| a == "-movflags").unwrap();
    args[at..end]
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn precise_args_for_openh264_use_a_bitrate_and_no_crf_or_preset() {
    let args = video_args(&precise_args(H264Encoder::OpenH264, 1920, 1080));
    assert_eq!(
        args,
        [
            "-c:v",
            "libopenh264",
            "-b:v",
            "6220800",
            "-pix_fmt",
            "yuv420p"
        ]
    );
    let mf = video_args(&precise_args(H264Encoder::MediaFoundation, 1920, 1080));
    assert_eq!(mf[..2], ["-c:v", "h264_mf"]);
    assert!(!mf.contains(&"-crf".to_string()) && !mf.contains(&"-preset".to_string()));
    let vt = video_args(&precise_args(H264Encoder::VideoToolbox, 1920, 1080));
    assert_eq!(vt[..4], ["-c:v", "h264_videotoolbox", "-b:v", "6220800"]);
}

#[test]
fn precise_args_for_x264_keep_the_existing_crf_and_preset() {
    let args = video_args(&precise_args(H264Encoder::X264, 1920, 1080));
    assert_eq!(
        args,
        ["-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt", "yuv420p"]
    );
}

#[test]
fn bitrate_is_clamped() {
    assert_eq!(video_bitrate_bps(320, 240), 1_500_000);
    assert_eq!(video_bitrate_bps(1920, 1080), 6_220_800);
    assert_eq!(video_bitrate_bps(3840, 2160), 20_000_000);
    assert_eq!(video_bitrate_bps(u32::MAX, u32::MAX), 20_000_000);
}
