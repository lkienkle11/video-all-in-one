use crate::{
    failure,
    render_thread::Renderer,
    software::{self, HEIGHT, WIDTH},
};
use gpui_kit::{Image, ImageFormat, RenderImage, Result, SvgRenderer};
use std::{
    future::poll_fn,
    sync::{mpsc, Arc, Mutex},
    task::{Poll, Waker},
    thread,
    time::Instant,
};

pub struct Frame {
    pub image: Arc<RenderImage>,
    pub position: f64,
    pub fps: f64,
}

#[derive(Default)]
pub struct Frames {
    latest: Option<Result<Frame, String>>,
    waker: Option<Waker>,
}

impl Frames {
    fn send(mailbox: &Mutex<Self>, frame: Result<Frame, String>) {
        let waker = {
            let mut slot = mailbox
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            slot.latest = Some(frame);
            slot.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    pub async fn next(mailbox: &Mutex<Self>) -> Result<Frame, String> {
        poll_fn(|cx| {
            let mut slot = mailbox
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(frame) = slot.latest.take() {
                Poll::Ready(frame)
            } else {
                slot.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await
    }
}

pub enum Control {
    TogglePause,
    Seek(i32),
    Stop,
}

pub struct Worker {
    commands: mpsc::Sender<Control>,
    thread: thread::Thread,
    join: Option<thread::JoinHandle<()>>,
}

impl Worker {
    pub fn start(fixture: String, mailbox: Arc<Mutex<Frames>>) -> Result<Self> {
        let (commands, receiver) = mpsc::channel();
        let join = thread::Builder::new()
            .name("mpv-control".into())
            .spawn(move || {
                if let Err(error) = playback(&fixture, receiver, &mailbox) {
                    Frames::send(&mailbox, Err(format!("{error:#}")));
                }
            })?;
        let thread = join.thread().clone();
        Ok(Self {
            commands,
            thread,
            join: Some(join),
        })
    }

    pub fn send(&self, command: Control) {
        let _ = self.commands.send(command);
        self.thread.unpark();
    }

    pub fn stop(mut self) {
        self.send(Control::Stop);
        if let Some(join) = self.join.take() {
            if join.join().is_err() {
                eprintln!("player-mpv: render thread panicked");
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.send(Control::Stop);
    }
}

fn playback(
    fixture: &str,
    commands: mpsc::Receiver<Control>,
    mailbox: &Mutex<Frames>,
) -> Result<()> {
    let mpv = Arc::new(software::player(false)?);
    let renderer = Renderer::start(mpv.clone(), true)?;
    mpv.command("loadfile", &[fixture, "replace"])?;
    let mut start = Instant::now();
    let mut count = 0_u64;
    let mut fps = 0.0;
    loop {
        while let Ok(command) = commands.try_recv() {
            match command {
                Control::Stop => return Ok(()),
                Control::TogglePause => mpv.command("cycle", &["pause"])?,
                Control::Seek(delta) => {
                    mpv.command("seek", &[&delta.to_string(), "relative+exact"])?
                }
            }
        }
        loop {
            let rendered = match renderer.frames.try_recv() {
                Ok(rendered) => rendered,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(failure("native render thread stopped").into())
                }
            };
            let rendered = rendered.map_err(failure)?;
            if let Ok(position) = mpv.get_property::<f64>("playback-time") {
                count += 1;
                let elapsed = start.elapsed().as_secs_f64();
                if elapsed >= 1.0 {
                    fps = count as f64 / elapsed;
                    count = 0;
                    start = Instant::now();
                }
                let image = rendered
                    .image
                    .ok_or_else(|| failure("GUI render has no image"))?;
                Frames::send(
                    mailbox,
                    Ok(Frame {
                        image,
                        position,
                        fps,
                    }),
                );
            }
        }
        // Both rendered-frame delivery and UI commands unpark this thread.
        // Tokens survive a wake before park; there is no timer/polling loop.
        thread::park();
    }
}

// GPUI Kit exports Image and its BMP decoder, but not image::Frame/RgbaImage.
// Keep the supplied two-dependency manifest: build an uncompressed BMP in
// memory, decode on the render worker, then move the resulting RenderImage Arc.
pub fn image(rgba: &[u8], svg: SvgRenderer) -> Result<Arc<RenderImage>> {
    if rgba.len() != WIDTH * HEIGHT * 4 {
        return Err(failure("invalid RGBA buffer length").into());
    }
    let mut bmp = vec![0_u8; 54 + rgba.len()];
    let size = bmp.len() as u32;
    bmp[0..2].copy_from_slice(b"BM");
    bmp[2..6].copy_from_slice(&size.to_le_bytes());
    bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&(WIDTH as i32).to_le_bytes());
    // Negative height is a top-down DIB; mpv writes top-left first.
    bmp[22..26].copy_from_slice(&(-(HEIGHT as i32)).to_le_bytes());
    bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&32_u16.to_le_bytes());
    for (source, target) in rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(bmp[54..].as_chunks_mut::<4>().0.iter_mut())
    {
        target.copy_from_slice(&[source[2], source[1], source[0], 255]);
    }
    Image::from_bytes(ImageFormat::Bmp, bmp).to_image_data(svg)
}
