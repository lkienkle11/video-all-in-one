//! Owns the native render thread and a single latest-frame slot.
use super::{backend, render_bridge::Software};
use libmpv2::Mpv;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Instant,
};
use vaio_core::ports::player::PlayerError;

#[derive(Clone)]
pub(super) struct Rendered {
    pub at: Instant,
    pub rgba: Arc<[u8]>,
}

type Slot = Arc<Mutex<Result<Option<Rendered>, PlayerError>>>;

pub(super) struct Renderer {
    latest: Slot,
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

impl Renderer {
    pub fn start(mpv: Arc<Mpv>, width: u32, height: u32) -> Result<Self, PlayerError> {
        let latest: Slot = Arc::new(Mutex::new(Ok(None)));
        let frames = latest.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name("mpv-render".into())
            .spawn(move || {
                let mut software = match Software::new(&mpv, width, height) {
                    Ok(renderer) => {
                        let _ = ready_tx.send(Ok(()));
                        renderer
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };
                while !stopped.load(Ordering::Acquire) {
                    // Timestamp before rendering so an in-flight pre-seek image
                    // cannot be accepted just because its copy finished later.
                    let at = Instant::now();
                    let result = software.render_if_ready().map(|ready| {
                        ready.then(|| Rendered {
                            at,
                            // Exactly one pixel copy; subsequent hand-offs clone the Arc.
                            rgba: Arc::from(software.rgba()),
                        })
                    });
                    let failed = result.is_err();
                    if !matches!(result, Ok(None)) {
                        match frames.lock() {
                            Ok(mut slot) => *slot = result,
                            Err(_) => break,
                        }
                    }
                    if failed {
                        break;
                    }
                    thread::park();
                }
            })
            .map_err(backend)?;
        let renderer = Self {
            latest,
            stop,
            join: Some(join),
        };
        // The RAII owner already exists: failed initialization also joins.
        ready_rx.recv().map_err(backend)??;
        Ok(renderer)
    }

    pub fn latest(&self) -> Result<Option<Rendered>, PlayerError> {
        self.latest.lock().map_err(backend)?.clone()
    }

    pub fn clear(&self) -> Result<(), PlayerError> {
        *self.latest.lock().map_err(backend)? = Ok(None);
        Ok(())
    }

    pub fn record_error(&self, error: PlayerError) {
        if let Ok(mut slot) = self.latest.lock() {
            *slot = Err(error);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            join.thread().unpark();
            if join.join().is_err() {
                eprintln!("vaio-media: native render thread panicked");
            }
        }
    }
}
