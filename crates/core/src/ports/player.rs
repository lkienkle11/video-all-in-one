//! The port the preview area needs from a video player. Adapters (libmpv in the media crate)
//! implement it; the core only sees decoded frames and times.

use crate::domain::media::MediaInfo;
use crate::domain::time::TimeMs;
use std::path::Path;
use std::sync::Arc;

/// Why a player call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerError {
    /// A call needed a loaded file and none is loaded.
    NotLoaded,
    /// The file cannot be played; the text names the file or the reason.
    Unsupported(String),
    /// A seek target lies after the end of the media.
    OutOfRange {
        /// The time the caller asked for.
        requested: TimeMs,
        /// The duration of the loaded media.
        duration: TimeMs,
    },
    /// The player library reported a failure.
    Backend(String),
}

/// One decoded picture in RGBA, 4 bytes per pixel, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The pixels; shared so a frame can reach the UI without a copy.
    pub rgba: Arc<[u8]>,
    /// The media time this picture belongs to.
    pub position: TimeMs,
}

impl Frame {
    /// Builds a frame, or `None` when the buffer is not exactly `width * height * 4` bytes.
    pub fn new(width: u32, height: u32, rgba: Arc<[u8]>, position: TimeMs) -> Option<Frame> {
        let expected = u64::from(width)
            .checked_mul(u64::from(height))?
            .checked_mul(4)?;
        (rgba.len() as u64 == expected).then_some(Frame {
            width,
            height,
            rgba,
            position,
        })
    }
}

/// Plays one local video file with sound and hands out pictures.
pub trait VideoPlayer {
    /// Opens a file and returns what is known about it; position goes to the start, paused.
    fn load(&mut self, path: &Path) -> Result<MediaInfo, PlayerError>;
    /// Starts or resumes playback with sound.
    fn play(&mut self);
    /// Pauses playback and keeps the position.
    fn pause(&mut self);
    /// Moves to an exact time; the frame at that time becomes current.
    fn seek(&mut self, to: TimeMs) -> Result<(), PlayerError>;
    /// The current media time.
    fn position(&self) -> TimeMs;
    /// The latest decoded picture, or `None` before a file is loaded or before the first frame.
    fn current_frame(&self) -> Option<Frame>;
}
