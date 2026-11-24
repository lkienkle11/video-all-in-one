mod frames;
mod gui;
mod headless;
mod render_thread;
mod software;

use gpui_kit::Result;
use std::{io, path::PathBuf};

fn failure(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("player-mpv: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let mut fixture = None;
    let mut headless = false;
    while let Some(arg) = args.next() {
        if arg == "--fixture" {
            fixture = Some(PathBuf::from(
                args.next().ok_or_else(|| failure("missing fixture"))?,
            ));
        } else if arg == "--headless" {
            headless = true;
        } else {
            return Err(failure("usage: player-mpv --fixture <path> [--headless]").into());
        }
    }
    let fixture = fixture
        .ok_or_else(|| failure("--fixture is required"))?
        .canonicalize()?;
    let fixture = fixture
        .to_str()
        .ok_or_else(|| failure("fixture path is not UTF-8"))?
        .to_owned();
    if headless {
        headless::run(&fixture)
    } else {
        gui::run(fixture);
        Ok(())
    }
}
