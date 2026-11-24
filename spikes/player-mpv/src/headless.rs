use crate::{failure, render_thread::Renderer, software};
use gpui_kit::Result;
use libmpv2::Mpv;
use std::{
    sync::{mpsc::RecvTimeoutError, Arc},
    time::{Duration, Instant},
};

fn next_frame(renderer: &Renderer, mpv: &Mpv, deadline: Instant) -> Result<Option<(Instant, f64)>> {
    loop {
        if Instant::now() >= deadline {
            return Ok(None);
        }
        match renderer
            .frames
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            Ok(rendered) => {
                let rendered = rendered.map_err(failure)?;
                // Sample playback time only after rendering. Exact seeks are paused,
                // so a settled target cannot advance while its buffer is inspected.
                // While paused no later frame follows, so a failed read must be
                // retried here instead of dropping the only matching frame.
                for _ in 0..50 {
                    if let Ok(pts) = mpv.get_property::<f64>("playback-time") {
                        return Ok(Some((rendered.at, pts)));
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
            Err(RecvTimeoutError::Timeout) => return Ok(None),
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(target_os = "macos")]
fn rss_mb() -> Result<f64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: usage is writable, aligned storage for the native rusage type.
    // getrusage initializes it on success and does not retain its pointer.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return Err(failure(format!(
            "getrusage(RUSAGE_SELF): {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    // SAFETY: the successful call above initialized usage.
    let usage = unsafe { usage.assume_init() };
    // Darwin reports peak resident bytes, not the current RSS reported by ps.
    Ok(usage.ru_maxrss as f64 / (1024.0 * 1024.0))
}

#[cfg(not(target_os = "macos"))]
fn rss_mb() -> Result<f64> {
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .map_err(|error| failure(format!("ps -o rss= -p {}: {error}", std::process::id())))?;
    if !output.status.success() {
        return Err(failure(format!(
            "ps RSS failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
        .into());
    }
    Ok(std::str::from_utf8(&output.stdout)?.trim().parse::<f64>()? / 1024.0)
}

pub fn run(fixture: &str) -> Result<()> {
    let mpv = Arc::new(software::player(true)?);
    let renderer = Renderer::start(mpv.clone(), false)?;
    let loaded = Instant::now();
    mpv.command("loadfile", &[fixture, "replace"])?;
    let (start, _) = next_frame(&renderer, &mpv, loaded + Duration::from_secs(15))?
        .ok_or_else(|| failure("no rendered video frame within 15 seconds"))?;
    let first_frame_ms = (start - loaded).as_secs_f64() * 1000.0;
    let deadline = start + Duration::from_secs(10);
    let mut frames_rendered = 1_u64;
    let mut previous = start;
    let mut max_gap = Duration::ZERO;
    while let Some((now, _)) = next_frame(&renderer, &mpv, deadline)? {
        if now > deadline {
            break;
        }
        frames_rendered += 1;
        max_gap = max_gap.max(now - previous);
        previous = now;
    }
    max_gap = max_gap.max(deadline.saturating_duration_since(previous));
    mpv.set_property("pause", true)?;
    let duration = mpv.get_property::<f64>("duration")?;
    if !duration.is_finite() || duration <= 0.0 {
        return Err(failure("invalid media duration").into());
    }
    let mut seeks = Vec::with_capacity(10);
    for index in 1..=10 {
        let target = duration * f64::from(index) / 11.0;
        // Discard pre-seek notifications; only a render completed after this
        // command can satisfy the measurement, even while the player is paused.
        while renderer.frames.try_recv().is_ok() {}
        let started = Instant::now();
        mpv.command("seek", &[&format!("{target:.6}"), "absolute+exact"])?;
        let deadline = started + Duration::from_secs(15);
        loop {
            match next_frame(&renderer, &mpv, deadline)? {
                Some((at, pts)) if at >= started && (pts - target).abs() <= 0.1 => {
                    // The render can land before mpv clears "seeking" and no later
                    // frame follows while paused, so poll the flag instead of
                    // waiting for another frame.
                    let mut settled_at = at;
                    while mpv.get_property::<bool>("seeking")? {
                        if Instant::now() >= deadline {
                            return Err(failure(format!(
                                "seek to {target:.6}s never cleared the seeking flag"
                            ))
                            .into());
                        }
                        std::thread::sleep(Duration::from_millis(1));
                        settled_at = Instant::now();
                    }
                    seeks.push((settled_at - started).as_secs_f64() * 1000.0);
                    break;
                }
                Some(_) => (),
                None => {
                    return Err(failure(format!(
                        "seek to {target:.6}s timed out without a matching rendered frame"
                    ))
                    .into())
                }
            }
        }
    }
    // Both version and OS error messages must escape every JSON control character.
    let escape = |value: &str| -> String {
        value
            .chars()
            .flat_map(|c| match c {
                '"' => "\\\"".chars().collect::<Vec<_>>(),
                '\\' => "\\\\".chars().collect(),
                c if c < ' ' => format!("\\u{:04x}", c as u32).chars().collect(),
                c => vec![c],
            })
            .collect()
    };
    // RSS is optional telemetry; failure must not discard successful playback.
    let (rss_json, rss_error) = match rss_mb() {
        Ok(value) => (format!("{value:.3}"), "null".into()),
        Err(error) => ("null".into(), format!("\"{}\"", escape(&error.to_string()))),
    };
    let version = escape(&mpv.get_property::<String>("mpv-version")?);
    let seek_max = seeks.iter().copied().fold(0.0, f64::max);
    println!("{{\"first_frame_ms\":{first_frame_ms:.3},\"frames_rendered\":{frames_rendered},\"avg_fps\":{:.3},\"max_frame_gap_ms\":{:.3},\"seek_ms\":{seeks:.3?},\"seek_max_ms\":{seek_max:.3},\"rss_mb\":{rss_json},\"rss_error\":{rss_error},\"libmpv_version\":\"{version}\"}}", frames_rendered as f64 / 10.0, max_gap.as_secs_f64() * 1000.0);
    Ok(())
}
