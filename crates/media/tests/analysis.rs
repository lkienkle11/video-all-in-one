use std::{path::PathBuf, process::Command};
use tokio_util::sync::CancellationToken;
use vaio_media::{analysis::analyze_source, ffmpeg::FfmpegPaths};

fn tools() -> FfmpegPaths {
    FfmpegPaths {
        ffmpeg: "ffmpeg".into(),
        ffprobe: "ffprobe".into(),
    }
}

fn path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("vaio-analysis-{}-{name}", std::process::id()))
}

#[tokio::test]
async fn analyzes_a_real_generated_fixture() {
    let path = path("nguồn video.mp4");
    let status = Command::new(&tools().ffmpeg)
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=10",
            "-t",
            "1",
            "-c:v",
            "libx264",
        ])
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    let info = analyze_source(&path, &tools(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(info.duration.0, 1000);
    assert_eq!(info.video.unwrap().width, 64);
}

#[tokio::test]
async fn non_media_and_unreadable_files_return_errors() {
    let path = path("not-media.txt");
    std::fs::write(&path, "not a media file").unwrap();
    assert!(analyze_source(&path, &tools(), CancellationToken::new())
        .await
        .is_err());
    assert!(analyze_source(
        &path.with_extension("absent"),
        &tools(),
        CancellationToken::new()
    )
    .await
    .is_err());
}

#[tokio::test]
async fn cancelled_analysis_returns_an_error() {
    let token = CancellationToken::new();
    token.cancel();
    assert!(analyze_source(&path("cancelled"), &tools(), token)
        .await
        .is_err());
}
