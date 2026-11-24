//! Headless-capable libmpv implementation of the preview player port.
mod metadata;
mod render_bridge;
mod render_thread;

use libmpv2::Mpv;
use render_thread::Renderer;
use std::{
    path::Path,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use vaio_core::{
    domain::{media::MediaInfo, time::TimeMs},
    ports::player::{Frame, PlayerError, VideoPlayer},
};

const SETTLE_TIMEOUT: Duration = Duration::from_secs(5);

/// Plays local video into a fixed RGBA surface; resizing is a later step.
///
/// Commands are synchronous and can wait up to five seconds for decoding.
/// Call them on a control worker, not a UI or async runtime worker. Native
/// rendering has its own thread, joined before this player is destroyed.
pub struct MpvPlayer {
    renderer: Renderer,
    mpv: Arc<Mpv>,
    width: u32,
    height: u32,
    info: Option<MediaInfo>,
}

impl MpvPlayer {
    /// Creates a player with real audio and a fixed output size.
    /// Returns a backend error for invalid dimensions or native initialization failure.
    pub fn new(width: u32, height: u32) -> Result<Self, PlayerError> {
        Self::create(width, height, true)
    }

    /// Configures audio before loading: `new(width, height)?.with_audio(false)?`
    /// selects mpv's null audio output without opening an audio device.
    /// Recreates the native context with the same dimensions; rejects a loaded player.
    pub fn with_audio(self, enabled: bool) -> Result<Self, PlayerError> {
        if self.info.is_some() {
            return Err(backend("audio must be configured before loading"));
        }
        Self::create(self.width, self.height, enabled)
    }

    fn create(width: u32, height: u32, audio: bool) -> Result<Self, PlayerError> {
        render_bridge::validate_size(width, height)?;
        let mpv = Arc::new(
            Mpv::with_initializer(|init| {
                init.set_option("config", "no")?;
                init.set_option("terminal", "no")?;
                init.set_option("hwdec", "auto-safe")?;
                init.set_option("vo", "libmpv")?;
                init.set_option("keep-open", "yes")?;
                init.set_option("pause", "yes")?;
                if !audio {
                    init.set_option("ao", "null")?;
                }
                Ok(())
            })
            .map_err(backend)?,
        );
        let renderer = Renderer::start(mpv.clone(), width, height)?;
        Ok(Self {
            renderer,
            mpv,
            width,
            height,
            info: None,
        })
    }

    fn wait_for_frame(&self, started: Instant, target: Option<TimeMs>) -> Result<(), PlayerError> {
        let deadline = started + SETTLE_TIMEOUT;
        loop {
            // A paused seek may render before seeking clears, with no further
            // notification. Retain that frame while polling the flag and time.
            if let Some(frame) = self.renderer.latest()? {
                let position = self
                    .mpv
                    .get_property::<f64>("playback-time")
                    .ok()
                    .and_then(TimeMs::from_secs_f64);
                let settled = !self.mpv.get_property::<bool>("seeking").map_err(backend)?;
                if frame.at >= started
                    && settled
                    && position.is_some_and(|position| {
                        target.is_none_or(|target| position.0.abs_diff(target.0) <= 40)
                    })
                {
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                return Err(backend("no settled post-command frame within 5 seconds"));
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}

impl VideoPlayer for MpvPlayer {
    fn load(&mut self, path: &Path) -> Result<MediaInfo, PlayerError> {
        self.info = None;
        self.mpv.command("stop", &[]).map_err(backend)?;
        self.renderer.clear()?;
        let path = path
            .canonicalize()
            .map_err(|error| PlayerError::Unsupported(error.to_string()))?;
        let path = path
            .to_str()
            .ok_or_else(|| PlayerError::Unsupported("path is not UTF-8".into()))?;
        self.mpv.set_property("pause", true).map_err(backend)?;
        let started = Instant::now();
        self.mpv
            .command("loadfile", &[path, "replace"])
            .map_err(|error| PlayerError::Unsupported(error.to_string()))?;
        self.wait_for_frame(started, None)
            .map_err(|error| PlayerError::Unsupported(format!("{path}: {error:?}")))?;
        let info = metadata::read(&self.mpv)?;
        self.info = Some(info.clone());
        Ok(info)
    }

    fn play(&mut self) {
        if self.info.is_some() {
            if let Err(error) = self.mpv.set_property("pause", false) {
                self.renderer.record_error(backend(error));
            }
        }
    }

    fn pause(&mut self) {
        if let Err(error) = self.mpv.set_property("pause", true) {
            self.renderer.record_error(backend(error));
        }
    }

    fn seek(&mut self, to: TimeMs) -> Result<(), PlayerError> {
        let duration = self.info.as_ref().ok_or(PlayerError::NotLoaded)?.duration;
        if to > duration {
            return Err(PlayerError::OutOfRange {
                requested: to,
                duration,
            });
        }
        // Freeze the target until its pixels and playback time agree, then
        // restore the caller's playback state even when the seek fails.
        let paused = self.mpv.get_property::<bool>("pause").map_err(backend)?;
        self.mpv.set_property("pause", true).map_err(backend)?;
        let started = Instant::now();
        let result = self
            .mpv
            .command(
                "seek",
                &[&format!("{:.6}", to.as_secs_f64()), "absolute+exact"],
            )
            .map_err(backend)
            .and_then(|()| self.wait_for_frame(started, Some(to)));
        let restored = self.mpv.set_property("pause", paused).map_err(backend);
        result.and(restored)
    }

    fn position(&self) -> TimeMs {
        if self.info.is_none() {
            return TimeMs(0);
        }
        self.mpv
            .get_property::<f64>("playback-time")
            .ok()
            .and_then(TimeMs::from_secs_f64)
            .unwrap_or(TimeMs(0))
    }

    fn current_frame(&self) -> Option<Frame> {
        self.info.as_ref()?;
        let rendered = self.renderer.latest().ok()??;
        Frame::new(self.width, self.height, rendered.rgba, self.position())
    }
}

fn backend(error: impl std::fmt::Display) -> PlayerError {
    PlayerError::Backend(error.to_string())
}
