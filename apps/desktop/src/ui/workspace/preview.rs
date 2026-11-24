//! Retains preview controls; native calls and bitmap conversion run on a Tokio blocking worker.
use crate::ui::settings::{label, SettingsView};
use gpui_kit::{
    component::{
        button::Button,
        slider::{Slider, SliderEvent, SliderState, SliderValue},
        ActiveTheme, Disableable, StyledExt,
    },
    div, img,
    prelude::*,
    AppContext, Context, Entity, Image, ImageFormat, IntoElement, ObjectFit, Render, RenderImage,
    Subscription, SvgRenderer, Task, TestSupportExt, Window,
};
use std::{
    path::PathBuf,
    sync::{mpsc, Arc},
    time::Duration,
};
use tokio::{runtime::Handle, sync::watch};
use vaio_core::{
    domain::time::TimeMs,
    i18n::{tr, Text},
    ports::player::{Frame, VideoPlayer},
};
use vaio_media::player::MpvPlayer;

#[derive(Clone)]
struct Snapshot {
    image: Option<Arc<RenderImage>>,
    position: TimeMs,
    duration: TimeMs,
    loaded: bool,
    playing: bool,
    failed: bool,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            image: None,
            position: TimeMs(0),
            duration: TimeMs(0),
            loaded: false,
            playing: false,
            failed: false,
        }
    }
}
enum Command {
    Toggle,
    Seek(TimeMs),
}

pub(super) struct PreviewView {
    runtime: Handle,
    audio: bool,
    settings: Entity<SettingsView>,
    selected: bool,
    /// True from the first user change until release: playback snapshots must not move the thumb.
    scrubbing: bool,
    snapshot: Snapshot,
    commands: Option<mpsc::Sender<Command>>,
    seek: Entity<SliderState>,
    receiver_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}
impl PreviewView {
    pub fn new(
        runtime: Handle,
        audio: bool,
        settings: Entity<SettingsView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let seek = cx.new(|_| SliderState::new().min(0.).max(1000.).step(1.));
        let subscription = cx.subscribe_in(&seek, window, |view, _, event, _, _| {
            // Release gives one seek per gesture. The worker also drops queued stale targets.
            if matches!(event, SliderEvent::Change(_)) {
                view.scrubbing = true;
            }
            if let SliderEvent::Release(SliderValue::Single(value)) = event {
                view.scrubbing = false;
                let target =
                    TimeMs((view.snapshot.duration.0 as f64 * f64::from(*value) / 1000.) as u64);
                if let Some(commands) = &view.commands {
                    let _ = commands.send(Command::Seek(target));
                }
            }
        });
        let language = cx.observe(&settings, |_, _, cx| cx.notify());
        Self {
            runtime,
            audio,
            settings,
            selected: false,
            scrubbing: false,
            snapshot: Snapshot::default(),
            commands: None,
            seek,
            receiver_task: None,
            _subscriptions: vec![subscription, language],
        }
    }

    pub fn load(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        // Replacing these drops the previous worker's command sender and UI receiver.
        self.commands = None;
        self.receiver_task = None;
        self.selected = true;
        self.scrubbing = false;
        self.snapshot = Snapshot::default();
        self.seek
            .update(cx, |seek, cx| seek.set_value(0., window, cx));
        let (commands, receiver) = mpsc::channel();
        let (sender, mut snapshots) = watch::channel(Snapshot::default());
        let svg = cx.svg_renderer();
        let audio = self.audio;
        self.runtime.spawn_blocking(move || {
            if let Err(error) = playback(path, audio, receiver, &sender, svg) {
                eprintln!("Preview failed: {error}");
                let _ = sender.send(Snapshot {
                    failed: true,
                    ..Snapshot::default()
                });
            }
        });
        self.commands = Some(commands);
        self.receiver_task = Some(cx.spawn_in(window, async move |view, cx| {
            while snapshots.changed().await.is_ok() {
                let snapshot = snapshots.borrow_and_update().clone();
                if let Err(error) = view.update_in(cx, |view, window, cx| {
                    if snapshot.duration.0 > 0 && !view.scrubbing {
                        let fraction = (snapshot.position.0 as f64 / snapshot.duration.0 as f64
                            * 1000.) as f32;
                        view.seek
                            .update(cx, |seek, cx| seek.set_value(fraction, window, cx));
                    }
                    view.snapshot = snapshot;
                    cx.notify();
                }) {
                    eprintln!("Preview receiver stopped: {error}");
                    break;
                }
            }
        }));
        cx.notify();
    }
}

impl Render for PreviewView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.settings.read(cx).language();
        div()
            .id("workspace-preview")
            .test_support()
            .v_flex()
            .size_full()
            .min_h_0()
            .min_w_0()
            .p_3()
            .gap_2()
            .child(label("preview-title", tr(language, Text::PreviewTitle)).font_semibold())
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .bg(cx.theme().muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(!self.selected, |this| {
                        this.child(label("preview-empty", tr(language, Text::PreviewEmpty)))
                    })
                    .when(self.selected && !self.snapshot.loaded, |this| {
                        this.child(label(
                            "preview-status",
                            tr(
                                language,
                                if self.snapshot.failed {
                                    Text::PreviewFailed
                                } else {
                                    Text::PreviewLoading
                                },
                            ),
                        ))
                    })
                    .when_some(self.snapshot.image.clone(), |this, image| {
                        this.child(img(image).size_full().object_fit(ObjectFit::Contain))
                    }),
            )
            .when(self.selected, |this| {
                this.child(
                    div()
                        .h_flex()
                        .gap_3()
                        .child(
                            Button::new("preview-play")
                                .label(tr(
                                    language,
                                    if self.snapshot.playing {
                                        Text::PreviewPause
                                    } else {
                                        Text::PreviewPlay
                                    },
                                ))
                                .disabled(!self.snapshot.loaded)
                                .on_click(cx.listener(|view, _, _, _| {
                                    if let Some(commands) = &view.commands {
                                        let _ = commands.send(Command::Toggle);
                                    }
                                })),
                        )
                        .child(
                            div()
                                .id("preview-seek")
                                .test_support()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    Slider::new(&self.seek)
                                        .disabled(!self.snapshot.loaded)
                                        .accessibility_label(tr(language, Text::PreviewSeek)),
                                ),
                        )
                        .child(label(
                            "preview-position",
                            format!("{} / {}", self.snapshot.position, self.snapshot.duration),
                        )),
                )
            })
    }
}

fn playback(
    path: PathBuf,
    audio: bool,
    receiver: mpsc::Receiver<Command>,
    sender: &watch::Sender<Snapshot>,
    svg: SvgRenderer,
) -> Result<(), String> {
    let mut player = MpvPlayer::new(640, 360).map_err(|error| format!("{error:?}"))?;
    if !audio {
        player = player
            .with_audio(false)
            .map_err(|error| format!("{error:?}"))?;
    }
    let info = player.load(&path).map_err(|error| format!("{error:?}"))?;
    let mut snapshot = Snapshot {
        loaded: true,
        duration: info.duration,
        ..Snapshot::default()
    };
    let mut pixels: Option<Arc<[u8]>> = None;
    loop {
        if let Some(frame) = player.current_frame() {
            if !pixels
                .as_ref()
                .is_some_and(|previous| Arc::ptr_eq(previous, &frame.rgba))
            {
                snapshot.image = Some(frame_image(&frame, svg.clone())?);
                pixels = Some(frame.rgba);
            }
        }
        snapshot.position = player.position();
        if sender.send(snapshot.clone()).is_err() {
            return Ok(());
        }
        let command = match receiver.recv_timeout(Duration::from_millis(33)) {
            Ok(command) => command,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        };
        let mut seek = None;
        for command in std::iter::once(command).chain(receiver.try_iter()) {
            match command {
                Command::Toggle => {
                    snapshot.playing = !snapshot.playing;
                    if snapshot.playing {
                        player.play();
                    } else {
                        player.pause();
                    }
                }
                Command::Seek(target) => seek = Some(target),
            }
        }
        if let Some(target) = seek {
            player.seek(target).map_err(|error| format!("{error:?}"))?;
        }
    }
}

/// GPUI Kit exposes the BMP decoder but not its image::Frame type.
/// Decode a top-down, uncompressed bitmap on the worker, as in the player spike.
fn frame_image(frame: &Frame, svg: SvgRenderer) -> Result<Arc<RenderImage>, String> {
    let mut bmp = vec![0_u8; 54 + frame.rgba.len()];
    let bytes = u32::try_from(bmp.len()).map_err(|error| error.to_string())?;
    let width = i32::try_from(frame.width).map_err(|error| error.to_string())?;
    let height = i32::try_from(frame.height).map_err(|error| error.to_string())?;
    bmp[0..2].copy_from_slice(b"BM");
    bmp[2..6].copy_from_slice(&bytes.to_le_bytes());
    bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&width.to_le_bytes());
    bmp[22..26].copy_from_slice(&(-height).to_le_bytes());
    bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&32_u16.to_le_bytes());
    for (source, target) in frame
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(bmp[54..].as_chunks_mut::<4>().0.iter_mut())
    {
        target.copy_from_slice(&[source[2], source[1], source[0], 255]);
    }
    Image::from_bytes(ImageFormat::Bmp, bmp)
        .to_image_data(svg)
        .map_err(|error| error.to_string())
}
