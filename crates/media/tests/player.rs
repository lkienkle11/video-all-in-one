use std::{
    path::PathBuf,
    process::Command,
    sync::{Arc, OnceLock},
    thread,
    time::{Duration, Instant},
};
use vaio_core::{
    domain::time::TimeMs,
    ports::player::{PlayerError, VideoPlayer},
};
use vaio_media::player::MpvPlayer;

fn fixture() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = std::env::temp_dir().join(format!("vaio-player-{}.mp4", std::process::id()));
        let output = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-nostdin",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x240:rate=25",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000",
                "-t",
                "8",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-g",
                "25",
                "-c:a",
                "aac",
            ])
            .arg(&path)
            .output()
            .expect("generate fixture with ffmpeg");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        path
    })
}

fn player() -> MpvPlayer {
    MpvPlayer::new(160, 120).unwrap().with_audio(false).unwrap()
}

#[test]
fn load_returns_duration_and_stream_info() {
    let mut player = player();
    let info = player.load(fixture()).unwrap();
    assert!(info.duration.0.abs_diff(8_000) <= 40, "{info:?}");
    let video = info.video.unwrap();
    assert_eq!((video.width, video.height), (320, 240));
    assert_eq!(video.codec, "h264");
    assert!(!info.container.is_empty());
    assert_eq!(info.audio.len(), 1);
    assert_eq!(info.audio[0].codec, "aac");
    assert_eq!(info.audio[0].channels, 1);
    assert!(player.position().0 <= 40);
    thread::sleep(Duration::from_millis(100));
    assert!(player.position().0 <= 40, "load must leave playback paused");
}

#[test]
fn seek_to_exact_time_sets_position_and_frame_dimensions() {
    let mut player = player();
    player.load(fixture()).unwrap();
    let started = Instant::now();
    player.seek(TimeMs(2_360)).unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(player.position().0.abs_diff(2_360) <= 40);
    let frame = player.current_frame().unwrap();
    assert_eq!((frame.width, frame.height), (160, 120));
    assert_eq!(frame.rgba.len(), 160 * 120 * 4);
    assert!(frame.position.0.abs_diff(2_360) <= 40);
    assert!(frame
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .all(|pixel| pixel[3] == 255));
    assert!(Arc::ptr_eq(
        &frame.rgba,
        &player.current_frame().unwrap().rgba
    ));
}

#[test]
fn frames_change_after_seek() {
    let mut player = player();
    player.load(fixture()).unwrap();
    player.seek(TimeMs(1_000)).unwrap();
    let first = player.current_frame().unwrap();
    player.seek(TimeMs(6_000)).unwrap();
    let second = player.current_frame().unwrap();
    assert_ne!(first.rgba, second.rgba, "seek must publish new pixels");
    player.seek(TimeMs(1_000)).unwrap();
    assert_eq!(first.rgba, player.current_frame().unwrap().rgba);
}

#[test]
fn play_advances_position_and_pause_stops_it() {
    let mut player = player();
    player.load(fixture()).unwrap();
    let before = player.position();
    player.play();
    let deadline = Instant::now() + Duration::from_secs(2);
    while player.position().0 <= before.0 + 200 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(player.position().0 > before.0 + 200);
    player.pause();
    let paused = player.position();
    thread::sleep(Duration::from_millis(180));
    assert!(player.position().0.abs_diff(paused.0) <= 40);
}

#[test]
fn seek_before_load_is_not_loaded_error() {
    let mut player = player();
    assert_eq!(player.seek(TimeMs(1_000)), Err(PlayerError::NotLoaded));
    assert!(player.current_frame().is_none());
}

#[test]
fn seek_past_the_end_is_out_of_range_error() {
    let mut player = player();
    let info = player.load(fixture()).unwrap();
    assert_eq!(
        player.seek(TimeMs(9_000)),
        Err(PlayerError::OutOfRange {
            requested: TimeMs(9_000),
            duration: info.duration,
        })
    );
}

#[test]
fn unsupported_file_is_an_error_not_a_panic() {
    let path = std::env::temp_dir().join(format!("vaio-not-video-{}.mp4", std::process::id()));
    std::fs::write(&path, "this is not a video").unwrap();
    let mut player = player();
    assert!(matches!(
        player.load(&path),
        Err(PlayerError::Unsupported(_))
    ));
    assert!(player.current_frame().is_none());
    assert_eq!(player.seek(TimeMs(0)), Err(PlayerError::NotLoaded));
}

#[test]
fn dropping_the_player_stops_the_render_thread() {
    let mut player = player();
    player.load(fixture()).unwrap();
    player.seek(TimeMs(1_000)).unwrap();
    let frame = player.current_frame().unwrap();
    let pixels = Arc::downgrade(&frame.rgba);
    drop(frame);
    assert!(pixels.upgrade().is_some());
    let started = Instant::now();
    drop(player);
    assert!(started.elapsed() < Duration::from_secs(5));
    // The render thread owns the latest-frame slot for its entire lifetime.
    // Its final buffer must be released synchronously, before Drop returns.
    assert!(
        pixels.upgrade().is_none(),
        "render thread still owns its frame slot"
    );
}
