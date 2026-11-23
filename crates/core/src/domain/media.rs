//! Facts about a media file that the export planner needs.

use crate::domain::time::TimeMs;

/// The video stream of a media file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoStream {
    /// The codec name as FFprobe reports it, such as `h264`.
    pub codec: String,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
}

/// One audio stream of a media file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioStream {
    /// The codec name as FFprobe reports it, such as `aac`.
    pub codec: String,
    /// The number of audio channels.
    pub channels: u32,
}

/// What the analysis found in a media file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaInfo {
    /// The total duration of the file.
    pub duration: TimeMs,
    /// The container format name as FFprobe reports it.
    pub container: String,
    /// The first video stream, when there is one.
    pub video: Option<VideoStream>,
    /// Every audio stream, in file order.
    pub audio: Vec<AudioStream>,
}
