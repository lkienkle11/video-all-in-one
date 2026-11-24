//! The external programs the media pipeline runs.

/// An external program run by the media pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    /// The FFmpeg encoder, muxer and cutter.
    Ffmpeg,
    /// The FFprobe media inspector.
    Ffprobe,
    /// The yt-dlp downloader.
    YtDlp,
}

impl Tool {
    /// The executable's file name without its platform suffix.
    pub fn file_stem(&self) -> &'static str {
        match self {
            Tool::Ffmpeg => "ffmpeg",
            Tool::Ffprobe => "ffprobe",
            Tool::YtDlp => "yt-dlp",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_file_stems_match_the_executable_names() {
        assert_eq!(Tool::Ffmpeg.file_stem(), "ffmpeg");
        assert_eq!(Tool::Ffprobe.file_stem(), "ffprobe");
        assert_eq!(Tool::YtDlp.file_stem(), "yt-dlp");
    }
}
