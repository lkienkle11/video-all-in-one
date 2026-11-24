//! Only this thread calls the render API. Control/property calls stay on its
//! caller, which may wait on mpv without preventing render progress.
use crate::{failure, frames, software::Software};
use gpui_kit::{RenderImage, Result, SvgRenderer};
use libmpv2::Mpv;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Instant,
};

pub struct Rendered {
    pub at: Instant,
    pub image: Option<Arc<RenderImage>>,
}

pub struct Renderer {
    pub frames: mpsc::Receiver<Result<Rendered, String>>,
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

impl Renderer {
    pub fn start(mpv: Arc<Mpv>, gui: bool) -> Result<Self> {
        // Headless messages contain only timestamps; GUI uses a bounded queue
        // so a slow property/command call cannot retain unlimited image buffers.
        let (headless_tx, headless_rx) = mpsc::channel();
        let (gui_tx, gui_rx) = mpsc::sync_channel(2);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let owner = thread::current();
        let join = thread::Builder::new()
            .name("mpv-render".into())
            .spawn(move || {
                let mut software = match Software::new(&mpv) {
                    Ok(renderer) => {
                        let _ = ready_tx.send(Ok(()));
                        renderer
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let svg = gui.then(|| SvgRenderer::new(Arc::new(())));
                while !stopped.load(Ordering::Acquire) {
                    let rendered = (|| -> Result<Option<Rendered>> {
                        if !software.render_if_ready()? {
                            return Ok(None);
                        }
                        let at = Instant::now();
                        let image = svg
                            .as_ref()
                            .map(|svg| frames::image(software.rgba(), svg.clone()))
                            .transpose()?;
                        Ok(Some(Rendered { at, image }))
                    })();
                    let message = match rendered {
                        Ok(Some(frame)) => Some(Ok(frame)),
                        Ok(None) => None,
                        Err(error) => Some(Err(error.to_string())),
                    };
                    if let Some(message) = message {
                        let failed = message.is_err();
                        if gui {
                            // Drop a frame rather than blocking the native renderer.
                            // A terminal error is also printed if the queue is full.
                            if let Err(error) = &message {
                                eprintln!("player-mpv: render: {error}");
                            }
                            let _ = gui_tx.try_send(message);
                        } else if headless_tx.send(message).is_err() {
                            break;
                        }
                        owner.unpark();
                        if failed {
                            break;
                        }
                    }
                    thread::park();
                }
            })?;
        ready_rx.recv()?.map_err(failure)?;
        Ok(Self {
            frames: if gui { gui_rx } else { headless_rx },
            stop,
            join: Some(join),
        })
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            join.thread().unpark();
            if join.join().is_err() {
                eprintln!("player-mpv: native render thread panicked");
            }
        }
    }
}
