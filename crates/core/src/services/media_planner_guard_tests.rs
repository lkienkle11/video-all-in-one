//! Tests for the planner's guards (paths, stem, segment bounds) and its output names.

use std::path::{Path, PathBuf};

use crate::domain::export::{CutMode, ExportMode, PlanError};
use crate::services::media_planner_fixtures::{full_info, plan, plan_stem, plan_with, seg};

#[test]
fn relative_source_starting_with_a_dash_is_rejected() {
    let info = full_info();
    let result = plan(
        &info,
        "-ss.mp4",
        &[seg(0, 1_000)],
        CutMode::Fast,
        ExportMode::Separate,
    );
    assert_eq!(
        result,
        Err(PlanError::NotAbsolute(PathBuf::from("-ss.mp4")))
    );
}

#[test]
fn outputs_never_equal_the_source() {
    let info = full_info();
    let separate = plan(
        &info,
        "/out/clip-01.mp4",
        &[seg(0, 1_000)],
        CutMode::Fast,
        ExportMode::Separate,
    );
    assert_eq!(separate, Err(PlanError::OutputEqualsSource));

    let merged = plan(
        &info,
        "/out/clip.mp4",
        &[seg(0, 1_000)],
        CutMode::Precise,
        ExportMode::Merged,
    );
    assert_eq!(merged, Err(PlanError::OutputEqualsSource));
}

#[test]
fn existing_output_gets_a_numbered_name() {
    let info = full_info();
    let one_taken = |p: &Path| p == Path::new("/out/clip-01.mp4");
    let plan_one = plan_with(
        &info,
        "/videos/in.mp4",
        &[seg(0, 1_000)],
        CutMode::Fast,
        ExportMode::Separate,
        &one_taken,
    )
    .unwrap();
    assert_eq!(
        plan_one.outputs,
        vec![PathBuf::from("/out/clip-01 (2).mp4")]
    );
    assert_eq!(
        plan_one.steps[0].temp_output,
        PathBuf::from("/out/clip-01 (2).partial.mp4")
    );

    let two_taken =
        |p: &Path| p == Path::new("/out/clip-01.mp4") || p == Path::new("/out/clip-01 (2).mp4");
    let plan_two = plan_with(
        &info,
        "/videos/in.mp4",
        &[seg(0, 1_000)],
        CutMode::Fast,
        ExportMode::Separate,
        &two_taken,
    )
    .unwrap();
    assert_eq!(
        plan_two.outputs,
        vec![PathBuf::from("/out/clip-01 (3).mp4")]
    );
}

#[test]
fn segment_beyond_duration_is_an_error() {
    let info = full_info();
    let past_end = plan(
        &info,
        "/videos/in.mp4",
        &[seg(100_000, 120_001)],
        CutMode::Fast,
        ExportMode::Separate,
    );
    assert_eq!(past_end, Err(PlanError::SegmentBeyondDuration));
}

#[test]
fn empty_segments_is_an_error() {
    let info = full_info();
    let none = plan(
        &info,
        "/videos/in.mp4",
        &[],
        CutMode::Fast,
        ExportMode::Merged,
    );
    assert_eq!(none, Err(PlanError::NoSegments));
}

#[test]
fn zero_length_or_reversed_segment_is_not_increasing() {
    let info = full_info();
    let zero = plan(
        &info,
        "/videos/in.mp4",
        &[seg(5_000, 5_000)],
        CutMode::Fast,
        ExportMode::Separate,
    );
    assert_eq!(zero, Err(PlanError::SegmentNotIncreasing));
}

#[test]
fn invalid_stems_are_rejected() {
    let rejected = [
        "", "a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b", ".hidden", "a\nb",
        "a\u{0}b", "a\tb",
    ];
    for stem in rejected {
        assert_eq!(
            plan_stem(stem),
            Err(PlanError::InvalidStem),
            "stem {stem:?}"
        );
    }
    let too_long = "x".repeat(101);
    assert_eq!(plan_stem(&too_long), Err(PlanError::InvalidStem));
}

#[test]
fn stem_of_exactly_one_hundred_characters_is_accepted() {
    let longest = "x".repeat(100);
    assert!(plan_stem(&longest).is_ok());
}

#[test]
fn temp_outputs_keep_the_extension() {
    let info = full_info();
    let precise = plan(
        &info,
        "/videos/in.mov",
        &[seg(0, 1_000)],
        CutMode::Precise,
        ExportMode::Separate,
    )
    .unwrap();
    assert!(precise.steps[0]
        .temp_output
        .to_string_lossy()
        .ends_with(".partial.mp4"));

    let fast_mkv = plan(
        &info,
        "/videos/in.MKV",
        &[seg(0, 1_000)],
        CutMode::Fast,
        ExportMode::Separate,
    )
    .unwrap();
    assert_eq!(fast_mkv.outputs, vec![PathBuf::from("/out/clip-01.mkv")]);
    assert_eq!(
        fast_mkv.steps[0].temp_output,
        PathBuf::from("/out/clip-01.partial.mkv")
    );

    let no_extension = plan(
        &info,
        "/videos/in",
        &[seg(0, 1_000)],
        CutMode::Fast,
        ExportMode::Separate,
    )
    .unwrap();
    assert_eq!(
        no_extension.outputs,
        vec![PathBuf::from("/out/clip-01.mkv")]
    );
}
