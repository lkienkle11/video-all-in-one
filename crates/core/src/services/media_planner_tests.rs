//! Tests for the planner's FFmpeg arguments, step shapes and concat list.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::domain::export::{CutMode, ExportMode, SupportFile};
use crate::domain::time::TimeMs;
use crate::domain::tool::Tool;
use crate::services::media_planner::{plan_export, ExportRequest};
use crate::services::media_planner_fixtures::{full_info, never, plan, seg, with_common};

#[test]
fn fast_plan_uses_stream_copy() {
    let info = full_info();
    let plan = plan(
        &info,
        "/videos/in.mp4",
        &[seg(10_000, 20_500)],
        CutMode::Fast,
        ExportMode::Separate,
    )
    .unwrap();

    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].program, Tool::Ffmpeg);
    assert_eq!(
        plan.steps[0].args,
        with_common(&[
            "-ss",
            "10.000",
            "-to",
            "20.500",
            "-i",
            "/videos/in.mp4",
            "-map",
            "0",
            "-c",
            "copy",
            "-avoid_negative_ts",
            "make_zero",
            "/out/clip-01.partial.mp4",
        ])
    );
    assert_eq!(
        plan.steps[0].temp_output,
        PathBuf::from("/out/clip-01.partial.mp4")
    );
    assert_eq!(
        plan.steps[0].final_output,
        PathBuf::from("/out/clip-01.mp4")
    );
    assert_eq!(plan.outputs, vec![PathBuf::from("/out/clip-01.mp4")]);
    assert!(plan.support_files.is_empty());
}

#[test]
fn precise_plan_reencodes() {
    let info = full_info();
    let plan = plan(
        &info,
        "/videos/in.mp4",
        &[seg(10_000, 20_500)],
        CutMode::Precise,
        ExportMode::Separate,
    )
    .unwrap();

    assert_eq!(
        plan.steps[0].args,
        with_common(&[
            "-ss",
            "10.000",
            "-to",
            "20.500",
            "-i",
            "/videos/in.mp4",
            "-map",
            "0:v:0",
            "-map",
            "0:a?",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-movflags",
            "+faststart",
            "/out/clip-01.partial.mp4",
        ])
    );
    assert_eq!(plan.outputs, vec![PathBuf::from("/out/clip-01.mp4")]);
}

#[test]
fn precise_plan_without_video_maps_audio_only() {
    let mut info = full_info();
    info.video = None;
    let plan = plan(
        &info,
        "/videos/in.mp4",
        &[seg(0, 5_000)],
        CutMode::Precise,
        ExportMode::Separate,
    )
    .unwrap();

    assert_eq!(
        plan.steps[0].args,
        with_common(&[
            "-ss",
            "0.000",
            "-to",
            "5.000",
            "-i",
            "/videos/in.mp4",
            "-map",
            "0:a",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-movflags",
            "+faststart",
            "/out/clip-01.partial.mp4",
        ])
    );
}

#[test]
fn precise_plan_without_audio_omits_the_audio_codec() {
    let mut info = full_info();
    info.audio = Vec::new();
    let plan = plan(
        &info,
        "/videos/in.mp4",
        &[seg(0, 5_000)],
        CutMode::Precise,
        ExportMode::Separate,
    )
    .unwrap();

    assert_eq!(
        plan.steps[0].args,
        with_common(&[
            "-ss",
            "0.000",
            "-to",
            "5.000",
            "-i",
            "/videos/in.mp4",
            "-map",
            "0:v:0",
            "-map",
            "0:a?",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            "/out/clip-01.partial.mp4",
        ])
    );
}

#[test]
fn separate_plan_has_one_step_per_segment() {
    let info = full_info();
    let plan = plan(
        &info,
        "/videos/in.mp4",
        &[seg(0, 1_000), seg(2_000, 3_000), seg(4_000, 5_000)],
        CutMode::Fast,
        ExportMode::Separate,
    )
    .unwrap();

    assert_eq!(plan.steps.len(), 3);
    assert_eq!(
        plan.outputs,
        vec![
            PathBuf::from("/out/clip-01.mp4"),
            PathBuf::from("/out/clip-02.mp4"),
            PathBuf::from("/out/clip-03.mp4"),
        ]
    );
    assert_eq!(
        plan.steps[1].final_output,
        PathBuf::from("/out/clip-02.mp4")
    );
    assert_eq!(
        plan.steps[1].temp_output,
        PathBuf::from("/out/clip-02.partial.mp4")
    );
    assert!(plan.steps[1].args.contains(&OsString::from("2.000")));
    assert!(plan.support_files.is_empty());
}

#[test]
fn merged_plan_ends_with_concat_step() {
    let info = full_info();
    let plan = plan(
        &info,
        "/videos/in.mp4",
        &[seg(0, 5_000), seg(10_000, 15_000)],
        CutMode::Fast,
        ExportMode::Merged,
    )
    .unwrap();

    assert_eq!(plan.steps.len(), 3);
    assert_eq!(
        plan.steps[0].args,
        with_common(&[
            "-ss",
            "0.000",
            "-to",
            "5.000",
            "-i",
            "/videos/in.mp4",
            "-map",
            "0",
            "-c",
            "copy",
            "-avoid_negative_ts",
            "make_zero",
            "/out/clip.piece-01.partial.mp4",
        ])
    );
    assert_eq!(
        plan.steps[0].final_output,
        PathBuf::from("/out/clip.piece-01.partial.mp4")
    );
    assert_eq!(
        plan.steps[0].temp_output,
        PathBuf::from("/out/clip.piece-01.partial.mp4")
    );

    let concat = &plan.steps[2];
    assert_eq!(concat.program, Tool::Ffmpeg);
    assert_eq!(
        concat.args,
        with_common(&[
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
            "/out/clip.concat.txt",
            "-c",
            "copy",
            "-movflags",
            "+faststart",
            "/out/clip.partial.mp4",
        ])
    );
    assert_eq!(concat.final_output, PathBuf::from("/out/clip.mp4"));
    assert_eq!(concat.temp_output, PathBuf::from("/out/clip.partial.mp4"));
    assert_eq!(plan.outputs, vec![PathBuf::from("/out/clip.mp4")]);
    assert_eq!(
        plan.support_files,
        vec![SupportFile {
            path: PathBuf::from("/out/clip.concat.txt"),
            contents:
                "file '/out/clip.piece-01.partial.mp4'\nfile '/out/clip.piece-02.partial.mp4'\n"
                    .to_string(),
        }]
    );
}

#[test]
fn merged_with_one_segment_has_one_piece_and_one_concat() {
    let info = full_info();
    let plan = plan(
        &info,
        "/videos/in.mp4",
        &[seg(0, 5_000)],
        CutMode::Precise,
        ExportMode::Merged,
    )
    .unwrap();

    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.support_files.len(), 1);
    assert_eq!(plan.outputs, vec![PathBuf::from("/out/clip.mp4")]);
    assert_eq!(
        plan.support_files[0].contents,
        "file '/out/clip.piece-01.partial.mp4'\n"
    );
}

#[test]
fn leading_dash_source_is_not_an_option() {
    let info = full_info();
    let plan = plan(
        &info,
        "/tmp/-ss.mp4",
        &[seg(0, 1_000)],
        CutMode::Fast,
        ExportMode::Separate,
    )
    .unwrap();

    let args = &plan.steps[0].args;
    let at = args.iter().position(|a| a.as_os_str() == "-i").unwrap();
    assert_eq!(args[at + 1], OsString::from("/tmp/-ss.mp4"));
    assert_eq!(
        args.iter()
            .filter(|a| a.as_os_str() == "/tmp/-ss.mp4")
            .count(),
        1
    );
}

#[test]
fn source_with_spaces_and_unicode_is_one_argument() {
    let info = full_info();
    let source = "/videos/My Clip é 日本.mp4";
    let plan = plan(
        &info,
        source,
        &[seg(0, 1_000)],
        CutMode::Precise,
        ExportMode::Separate,
    )
    .unwrap();

    let args = &plan.steps[0].args;
    let at = args.iter().position(|a| a.as_os_str() == "-i").unwrap();
    assert_eq!(args[at + 1], OsString::from(source));
    assert_eq!(args.iter().filter(|a| a.as_os_str() == source).count(), 1);
}

#[test]
fn concat_list_escapes_single_quotes() {
    let info = full_info();
    let segments = [seg(0, 1_000)];
    let request = ExportRequest {
        info: &info,
        source: Path::new("/videos/in.mp4"),
        segments: &segments,
        cut: CutMode::Fast,
        export: ExportMode::Merged,
        out_dir: Path::new("/out"),
        stem: "it's",
    };
    let plan = plan_export(&request, &never).unwrap();

    assert_eq!(
        plan.support_files,
        vec![SupportFile {
            path: PathBuf::from("/out/it's.concat.txt"),
            contents: "file '/out/it'\\''s.piece-01.partial.mp4'\n".to_string(),
        }]
    );
}

#[test]
fn time_is_written_with_three_fraction_digits() {
    let mut info = full_info();
    info.duration = TimeMs(4_000_000);
    let zero = plan(
        &info,
        "/videos/in.mp4",
        &[seg(0, 90_500)],
        CutMode::Fast,
        ExportMode::Separate,
    )
    .unwrap();
    assert_eq!(
        zero.steps[0].args[8..12].to_vec(),
        ["-ss", "0.000", "-to", "90.500"]
            .map(OsString::from)
            .to_vec()
    );

    let late = plan(
        &info,
        "/videos/in.mp4",
        &[seg(3_723_004, 3_999_999)],
        CutMode::Fast,
        ExportMode::Separate,
    )
    .unwrap();
    assert_eq!(late.steps[0].args[8], OsString::from("-ss"));
    assert_eq!(late.steps[0].args[9], OsString::from("3723.004"));
    assert_eq!(late.steps[0].args[11], OsString::from("3999.999"));
}
