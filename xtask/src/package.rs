//! `cargo xtask package`: turns a built production binary into the installer and the update
//! artifact of one operating system. The pure parts (names, manifests, scripts) are unit
//! tested; the tool calls (`otool`, `makensis`, `dpkg-deb`, ...) run only on their own OS.

mod deb;
pub(crate) mod elf;
pub(crate) mod license;
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
    /// macOS and Ubuntu: the folder of our own libmpv build.
    pub libmpv_dir: Option<PathBuf>,
    /// The CPU of a cross-compiled executable (`aarch64` or `x86_64`); this machine's when unset.
    pub arch: Option<String>,
    /// Lets GPL or non-free libraries into the package (local testing only).
    pub allow_gpl: bool,
}

/// The CPU the package is for: the `--arch` value, or this machine's.
pub fn effective_arch(arg: Option<&str>) -> &str {
    arg.unwrap_or(std::env::consts::ARCH)
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
    let key = platform_key(options.target, effective_arch(options.arch.as_deref()))?;
    match options.target {
        Target::Macos => macos::build(
            root,
            &binary,
            &out,
            &version,
            key,
            options.allow_gpl,
            options.libmpv_dir.as_deref(),
        )?,
        Target::Windows => nsis::build(
            root,
            &binary,
            &out,
            &version,
            key,
            options.libs.as_deref(),
            options.allow_gpl,
        )?,
        // The Ubuntu package bundles our own libmpv, because Ubuntu 22.04 has only the old one.
        Target::Ubuntu => {
            let libmpv_dir = options.libmpv_dir.as_deref().context(
                "Ubuntu packages bundle our own libmpv: pass --libmpv-dir (see docs/RELEASING.md)",
            )?;
            deb::build(
                root,
                &binary,
                &out,
                &version,
                key,
                &deb::Bundle {
                    libmpv_dir,
                    allow_gpl: options.allow_gpl,
                },
            )?
        }
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

/// The committed production icon `file`; the packagers refuse to build without it.
pub(crate) fn production_icon(root: &Path, file: &str) -> Result<PathBuf> {
    let path = crate::icons::generated_path(root, crate::icons::Variant::Production, file);
    if !path.is_file() {
        bail!("{} is missing; run `cargo xtask icons`", path.display());
    }
    Ok(path)
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

#[cfg(test)]
mod arch_tests {
    use super::{effective_arch, platform_key, Target};

    #[test]
    fn an_explicit_arch_decides_the_platform_key() {
        assert_eq!(effective_arch(Some("x86_64")), "x86_64");
        assert_eq!(effective_arch(None), std::env::consts::ARCH);
        assert_eq!(
            platform_key(Target::Macos, effective_arch(Some("x86_64"))).unwrap(),
            "macos-x86_64"
        );
        assert_eq!(
            platform_key(Target::Macos, effective_arch(Some("aarch64"))).unwrap(),
            "macos-aarch64"
        );
    }
}
