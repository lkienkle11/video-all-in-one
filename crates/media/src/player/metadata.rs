//! Translates native track properties into the core's media description.
use super::backend;
use libmpv2::Mpv;
use vaio_core::{
    domain::{
        media::{AudioStream, MediaInfo, VideoStream},
        time::TimeMs,
    },
    ports::player::PlayerError,
};

pub(super) fn read(mpv: &Mpv) -> Result<MediaInfo, PlayerError> {
    let duration = TimeMs::from_secs_f64(mpv.get_property("duration").map_err(backend)?)
        .ok_or_else(|| backend("invalid media duration"))?;
    let mut info = MediaInfo {
        duration,
        container: mpv.get_property("file-format").map_err(backend)?,
        video: None,
        audio: Vec::new(),
    };
    let count: i64 = mpv.get_property("track-list/count").map_err(backend)?;
    for index in 0..count {
        let prefix = format!("track-list/{index}");
        let kind: String = mpv
            .get_property(&format!("{prefix}/type"))
            .map_err(backend)?;
        match kind.as_str() {
            "video" if info.video.is_none() => {
                info.video = Some(VideoStream {
                    codec: mpv
                        .get_property(&format!("{prefix}/codec"))
                        .map_err(backend)?,
                    width: dimension(mpv, &format!("{prefix}/demux-w"))?,
                    height: dimension(mpv, &format!("{prefix}/demux-h"))?,
                });
            }
            "audio" => info.audio.push(AudioStream {
                codec: mpv
                    .get_property(&format!("{prefix}/codec"))
                    .map_err(backend)?,
                channels: dimension(mpv, &format!("{prefix}/demux-channel-count"))?,
            }),
            _ => (),
        }
    }
    Ok(info)
}

fn dimension(mpv: &Mpv, name: &str) -> Result<u32, PlayerError> {
    u32::try_from(mpv.get_property::<i64>(name).map_err(backend)?).map_err(backend)
}
