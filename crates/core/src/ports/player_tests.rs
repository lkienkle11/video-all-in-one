use super::player::{Frame, PlayerError, VideoPlayer};
use crate::domain::media::MediaInfo;
use crate::domain::time::TimeMs;
use std::path::Path;
use std::sync::Arc;

/// A player that holds a fake file of a fixed length and renders no real pixels.
struct FakePlayer {
    duration: Option<TimeMs>,
    position: TimeMs,
    playing: bool,
}

impl FakePlayer {
    fn new() -> Self {
        Self {
            duration: None,
            position: TimeMs(0),
            playing: false,
        }
    }
}

impl VideoPlayer for FakePlayer {
    fn load(&mut self, path: &Path) -> Result<MediaInfo, PlayerError> {
        if path.extension().is_none() {
            return Err(PlayerError::Unsupported(path.display().to_string()));
        }
        self.duration = Some(TimeMs(60_000));
        self.position = TimeMs(0);
        Ok(MediaInfo {
            duration: TimeMs(60_000),
            container: "fake".into(),
            video: None,
            audio: vec![],
        })
    }
    fn play(&mut self) {
        self.playing = true;
    }
    fn pause(&mut self) {
        self.playing = false;
    }
    fn seek(&mut self, to: TimeMs) -> Result<(), PlayerError> {
        let duration = self.duration.ok_or(PlayerError::NotLoaded)?;
        if to > duration {
            return Err(PlayerError::OutOfRange {
                requested: to,
                duration,
            });
        }
        self.position = to;
        Ok(())
    }
    fn position(&self) -> TimeMs {
        self.position
    }
    fn current_frame(&self) -> Option<Frame> {
        self.duration?;
        Some(Frame {
            width: 1,
            height: 1,
            rgba: Arc::from(vec![0u8; 4]),
            position: self.position,
        })
    }
}

#[test]
fn fake_player_seek_updates_position() {
    let mut player = FakePlayer::new();
    player.load(Path::new("/media/clip.mp4")).unwrap();
    player.seek(TimeMs(12_500)).unwrap();
    assert_eq!(player.position(), TimeMs(12_500));
    assert_eq!(player.current_frame().unwrap().position, TimeMs(12_500));
}

#[test]
fn seek_before_load_is_refused() {
    let mut player = FakePlayer::new();
    assert_eq!(player.seek(TimeMs(1)), Err(PlayerError::NotLoaded));
    assert!(player.current_frame().is_none());
}

#[test]
fn seek_past_the_end_is_refused_and_keeps_position() {
    let mut player = FakePlayer::new();
    player.load(Path::new("/media/clip.mp4")).unwrap();
    player.seek(TimeMs(5)).unwrap();
    let error = player.seek(TimeMs(60_001)).unwrap_err();
    assert_eq!(
        error,
        PlayerError::OutOfRange {
            requested: TimeMs(60_001),
            duration: TimeMs(60_000)
        }
    );
    assert_eq!(player.position(), TimeMs(5));
}

#[test]
fn unsupported_file_fails_to_load() {
    let mut player = FakePlayer::new();
    assert!(matches!(
        player.load(Path::new("/media/noext")),
        Err(PlayerError::Unsupported(_))
    ));
}

#[test]
fn frame_checks_its_buffer_length() {
    assert!(Frame::new(2, 2, Arc::from(vec![0u8; 16]), TimeMs(0)).is_some());
    assert!(Frame::new(2, 2, Arc::from(vec![0u8; 15]), TimeMs(0)).is_none());
    assert!(Frame::new(2, 2, Arc::from(vec![0u8; 17]), TimeMs(0)).is_none());
    assert!(Frame::new(u32::MAX, u32::MAX, Arc::from(vec![0u8; 4]), TimeMs(0)).is_none());
}
