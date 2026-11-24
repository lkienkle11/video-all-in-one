//! `cargo xtask package`: turns a built production binary into the installer and the update
//! artifact of one operating system. The pure parts (names, manifests, scripts) are unit
//! tested; the tool calls (`otool`, `makensis`, `dpkg-deb`, ...) run only on their own OS.

mod deb;
mod macos;
mod macos_libs;
mod nsis;
#[cfg(test)]
mod tests;

use crate::release::changelog::{read_notes, Notes};
use crate::release::latest_json::{FILE_PREFIX, PLATFORMS};
use crate::release::set_version::{plain_version, workspace_version};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The name users see.
pub(crate) const APP_NAME: &str = "Video All-in-One";
/// The executable name inside every package.
pub(crate) const BINARY_NAME: &str = "video-all-in-one";
/// The production bundle identifier (see `vaio_platform::channel`).
pub(crate) const IDENTIFIER: &str = "app.videoallinone";

/// The operating system a package is built for. Each one can only be built on itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Target {
    Macos,
    Windows,
    Ubuntu,
}

/// What `cargo xtask package` was asked to do.
pub struct Options {
    pub target: Target,
    pub version: Option<String>,
    pub out: Option<PathBuf>,
    pub binary: Option<PathBuf>,
    pub libs: Option<PathBuf>,
}

/// The update platform key of `target` on a machine with CPU `arch` (`std::env::consts::ARCH`).
pub fn platform_key(target: Target, arch: &str) -> Result<&'static str> {
    match (target, arch) {
        (Target::Macos, "aarch64") => Ok("macos-aarch64"),
        (Target::Macos, "x86_64") => Ok("macos-x86_64"),
        (Target::Windows, "x86_64") => Ok("windows-x86_64"),
        (Target::Ubuntu, "x86_64") => Ok("linux-x86_64"),
        _ => bail!("no package for {target:?} on a {arch} machine"),
    }
}

/// The extension of the update artifact of `platform_key`.
pub fn update_extension(platform_key: &str) -> Result<&'static str> {
    PLATFORMS
        .iter()
        .find(|(key, _)| *key == platform_key)
        .map(|(_, ext)| *ext)
        .with_context(|| format!("unknown platform key `{platform_key}`"))
}

/// `video-all-in-one-<version>-<platform_key>.<ext>`, the pattern `latest-json` matches.
pub fn package_file_name(version: &str, platform_key: &str, ext: &str) -> String {
    format!("{FILE_PREFIX}{version}-{platform_key}.{ext}")
}

/// The notes of `version` in `changelog`, which must have both languages and no placeholder.
pub fn check_changelog(changelog: &str, version: &str) -> Result<Notes> {
    plain_version(version)?;
    read_notes(changelog, version)
}

/// Builds the installer and the update artifact described by `options` under the output folder.
/// Nothing is built when the version, the changelog or the binary is wrong.
pub fn run(root: &Path, options: &Options) -> Result<()> {
    let host = match options.target {
        Target::Macos => cfg!(target_os = "macos"),
        Target::Windows => cfg!(target_os = "windows"),
        Target::Ubuntu => cfg!(target_os = "linux"),
    };
    if !host {
        bail!(
            "{:?} packages can only be built on that operating system",
            options.target
        );
    }
    let version = match &options.version {
        Some(version) => version.clone(),
        None => {
            let cargo =
                std::fs::read_to_string(root.join("Cargo.toml")).context("reading Cargo.toml")?;
            workspace_version(&cargo)?
        }
    };
    let changelog =
        std::fs::read_to_string(root.join("CHANGELOG.md")).context("reading CHANGELOG.md")?;
    check_changelog(&changelog, &version)?;
    let exe = if cfg!(windows) {
        "vaio-desktop.exe"
    } else {
        "vaio-desktop"
    };
    let binary = options
        .binary
        .clone()
        .unwrap_or_else(|| root.join("target/release").join(exe));
    if !binary.is_file() {
        bail!(
            "{} does not exist; build it first with `cargo build --release -p vaio-desktop --features production`",
            binary.display()
        );
    }
    let out = options
        .out
        .clone()
        .unwrap_or_else(|| root.join("target/package"));
    std::fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
    let key = platform_key(options.target, std::env::consts::ARCH)?;
    match options.target {
        Target::Macos => macos::build(&binary, &out, &version, key)?,
        Target::Windows => nsis::build(&binary, &out, &version, key, options.libs.as_deref())?,
        Target::Ubuntu => deb::build(&binary, &out, &version, key)?,
    }
    let artifact = out.join(package_file_name(&version, key, update_extension(key)?));
    if !artifact.is_file() {
        bail!(
            "packaging finished but {} was not produced",
            artifact.display()
        );
    }
    println!("update artifact: {}", artifact.display());
    Ok(())
}

/// Runs a tool and fails with its name and exit status when it does not succeed.
pub(crate) fn run_tool(command: &mut Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command
        .status()
        .with_context(|| format!("starting {program}"))?;
    if !status.success() {
        bail!("{program} failed with {status}");
    }
    Ok(())
}
