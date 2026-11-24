//! `cargo xtask libmpv`: builds our own LGPL libmpv (with the FFmpeg and the few libraries it
//! needs) from pinned sources, and packs the result. Release packages bundle only this build:
//! the system libmpv of an Ubuntu is too old (22.04) and Homebrew's builds are GPL.

mod fetch;
mod inspect;
mod layout;
mod manifest;
mod pack;
mod recipe;
pub(crate) mod sources;
mod steps;

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub(crate) use sources::RECIPE_VERSION;

/// The lowest macOS the libraries are built for (the minimum the app promises).
pub(crate) const MACOS_DEPLOYMENT_TARGET: &str = "13.0";

/// The operating system a build is made for. Each one can only be built on itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Target {
    Ubuntu,
    Macos,
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// Downloads the pinned sources, builds libmpv and its libraries as LGPL, and checks them.
    Build {
        #[arg(long, value_enum)]
        target: Target,
        /// The folder to build into; it must not exist yet or be empty.
        #[arg(long)]
        out: PathBuf,
    },
    /// Packs a finished build folder into a `.tar.gz` and prints its SHA-256.
    Pack {
        #[arg(long)]
        dir: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

/// Runs one `cargo xtask libmpv` command.
pub fn run(command: Command) -> Result<()> {
    match command {
        Command::Build { target, out } => build(target, &out),
        Command::Pack { dir, out } => {
            let hash = pack::pack(&dir, &out)?;
            let name = out
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            println!("{hash}  {name}");
            Ok(())
        }
    }
}

fn build(target: Target, out: &Path) -> Result<()> {
    let host = match target {
        Target::Ubuntu => cfg!(target_os = "linux"),
        Target::Macos => cfg!(target_os = "macos"),
    };
    if !host {
        bail!("{target:?} libraries can only be built on that operating system");
    }
    let missing = steps::missing_tools(target);
    if !missing.is_empty() {
        bail!(
            "these build tools are not installed: {}. Install them first (see \"Our own libmpv\" in docs/RELEASING.md).",
            missing.join(", ")
        );
    }
    if out.is_dir() && std::fs::read_dir(out)?.next().is_some() {
        bail!("{} is not empty; build into a new folder", out.display());
    }
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let out = out.canonicalize()?;
    let work = out.with_file_name(format!(
        "{}-work",
        out.file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    ));
    std::fs::create_dir_all(&work)?;
    let env = steps::clean_env(target, &out, &|name| std::env::var(name).ok());
    let built = steps::build_all(target, &out, &work, &env)?;
    layout::finish(target, &out, &work, &built, &env)?;
    println!(
        "libmpv recipe {RECIPE_VERSION} built into {}",
        out.display()
    );
    Ok(())
}
