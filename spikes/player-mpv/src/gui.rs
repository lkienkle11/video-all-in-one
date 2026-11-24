use crate::frames::{Control, Frames, Worker};
use gpui_kit::{component::ActiveTheme, prelude::FluentBuilder, *};
use std::sync::{Arc, Mutex};

struct Player {
    image: Option<Arc<RenderImage>>,
    position: f64,
    fps: f64,
    error: Option<String>,
    focus: FocusHandle,
    worker: Option<Worker>,
    _updates: Task<()>,
}

impl Player {
    fn new(fixture: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mailbox = Arc::new(Mutex::new(Frames::default()));
        let (worker, error) = match Worker::start(fixture, mailbox.clone()) {
            Ok(worker) => (Some(worker), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let updates = cx.spawn_in(window, async move |view, cx| loop {
            let frame = Frames::next(&mailbox).await;
            let failed = frame.is_err();
            if cx
                .update(|window, cx| {
                    view.update(cx, |this, cx| {
                        match frame {
                            Ok(frame) => {
                                if let Some(old) = this.image.replace(frame.image) {
                                    if let Err(error) = window.drop_image(old) {
                                        this.error = Some(error.to_string());
                                    }
                                }
                                this.position = frame.position;
                                this.fps = frame.fps;
                            }
                            Err(error) => this.error = Some(error),
                        }
                        cx.notify();
                    })
                })
                .is_err()
                || failed
            {
                break;
            }
        });
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            image: None,
            position: 0.0,
            fps: 0.0,
            error,
            focus,
            worker,
            _updates: updates,
        }
    }
}

impl Render for Player {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_3()
            .gap_2()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let command = match event.keystroke.key.as_str() {
                    "space" => Some(Control::TogglePause),
                    "left" => Some(Control::Seek(-5)),
                    "right" => Some(Control::Seek(5)),
                    "escape" => {
                        cx.quit();
                        None
                    }
                    _ => None,
                };
                if let (Some(worker), Some(command)) = (&this.worker, command) {
                    worker.send(command);
                }
            }))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .when_some(self.image.clone(), |view, image| {
                        view.child(img(image).size_full().object_fit(ObjectFit::Contain))
                    }),
            )
            .child(format!("{:.3} s   ·   {:.1} fps", self.position, self.fps))
            .when_some(self.error.clone(), |view, error| view.child(error))
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // Join after the window is closing, never from a playback/seek handler.
        if let Some(worker) = self.worker.take() {
            worker.stop();
        }
    }
}

pub fn run(fixture: String) {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            if let Err(error) =
                gpui_kit::open_window(WindowOptions::default(), cx, move |window, cx| {
                    cx.new(|cx| Player::new(fixture, window, cx))
                })
            {
                eprintln!("player-mpv: window: {error:#}");
                cx.quit();
            }
            cx.activate(true);
        });
}
