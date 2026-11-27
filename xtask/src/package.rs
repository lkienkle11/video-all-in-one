//! `cargo xtask package`: turns a built production binary into the installer and the update
//! artifact of one operating system. The pure parts (names, manifests, scripts) are unit
//! tested; the tool calls (`otool`, `makensis`, `dpkg-deb`, ...) run only on their own OS.

mod deb;
pub(crate) mod elf;
pub(crate) mod license;
mod macos;
mod macos_libs;
mod nsis;
pub(crate) mod pe;
#[cfg(test)]
mod tests;

use crate::release::changelog::{read_notes, Notes};
use crate::release::latest_json::{FILE_PREFIX, PLATFORMS};
use crate::release::set_version::{plain_version, workspace_version};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::release::latest_json::DEV_MARKER;
use vaio_platform::channel::Channel;

/// The build channel of a package (`--channel`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum BuildChannel {
    Production,
    Dev,
}

impl From<BuildChannel> for Channel {
    fn from(channel: BuildChannel) -> Channel {
        match channel {
            BuildChannel::Production => Channel::Production,
            BuildChannel::Dev => Channel::Development,
        }
    }
}

/// What one package is: its channel, version and platform key.
#[derive(Clone, Copy)]
pub(crate) struct Release<'a> {
    pub channel: Channel,
    pub version: &'a str,
    pub key: &'a str,
}

impl<'a> Release<'a> {
    /// A production package.
    #[cfg(test)]
    pub(crate) fn production(version: &'a str, key: &'a str) -> Self {
        Release {
            channel: Channel::Production,
            version,
            key,
        }
    }
}

impl Release<'_> {
    /// The name of this package's file with extension `ext`.
    pub(crate) fn file_name(&self, ext: &str) -> String {
        package_file_name(self.channel, self.version, self.key, ext)
    }
}

/// The name of a staging folder: `stage-<os>`, with `-dev` for a development build, so a
/// development package never clears the staged files of a production one.
pub(crate) fn stage_name(channel: Channel, os: &str) -> String {
    match channel {
        Channel::Production => format!("stage-{os}"),
        Channel::Development => format!("stage-{os}-dev"),
    }
}

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
    /// Windows: the folder with ffmpeg.exe, ffprobe.exe and their DLLs, when not in `libs`.
    pub tools: Option<PathBuf>,
    /// macOS and Ubuntu: the folder of our own libmpv build.
    pub libmpv_dir: Option<PathBuf>,
    /// The CPU of a cross-compiled executable (`aarch64` or `x86_64`); this machine's when unset.
    pub arch: Option<String>,
    /// `dev` builds the development installer only (no update artifact, never signed).
    pub channel: BuildChannel,
    /// Lets GPL or non-free libraries into the package (local testing only).
    pub allow_gpl: bool,
}

/// `path` made absolute without touching the file system (and without the `\\?\` prefix that
/// `canonicalize` adds on Windows, which `makensis` does not accept).
pub fn absolute_out(path: &Path) -> Result<PathBuf> {
    std::path::absolute(path).with_context(|| format!("making {} absolute", path.display()))
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

/// `video-all-in-one-<version>-<platform_key>.<ext>`, the pattern `latest-json` matches; a
/// development build has `-dev-` between the version and the platform key, which it ignores.
pub fn package_file_name(channel: Channel, version: &str, platform_key: &str, ext: &str) -> String {
    match channel {
        Channel::Production => format!("{FILE_PREFIX}{version}-{platform_key}.{ext}"),
        Channel::Development => {
            format!("{FILE_PREFIX}{version}{DEV_MARKER}{platform_key}.{ext}")
        }
    }
}

/// The extension of the file that must exist when packaging ends: the update artifact of a
/// production build, the installer of a development build (which has no update artifact).
fn final_extension(channel: Channel, target: Target, platform_key: &str) -> Result<&'static str> {
    match (channel, target) {
        (Channel::Production, _) => update_extension(platform_key),
        (Channel::Development, Target::Macos) => Ok("dmg"),
        (Channel::Development, Target::Windows) => Ok("exe"),
        (Channel::Development, Target::Ubuntu) => Ok("deb"),
    }
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
            "{} does not exist; build it first with `cargo build --release -p vaio-desktop --features production` (a development package: without `--features production`)",
            binary.display()
        );
    }
    let out = options
        .out
        .clone()
        .unwrap_or_else(|| root.join("target/package"));
    std::fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
    // An absolute folder: makensis reads relative paths of the script from the script's own
    // folder, so a relative `--out dist` made it look for `dist\dist\icon.ico`.
    let out = absolute_out(&out)?;
    let key = platform_key(options.target, effective_arch(options.arch.as_deref()))?;
    let channel = Channel::from(options.channel);
    let release = Release {
        channel,
        version: &version,
        key,
    };
    match options.target {
        Target::Macos => macos::build(
            root,
            &binary,
            &out,
            &release,
            options.allow_gpl,
            options.libmpv_dir.as_deref(),
        )?,
        Target::Windows => nsis::build(
            root,
            &binary,
            &out,
            &release,
            options.libs.as_deref(),
            options.tools.as_deref(),
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
                &release,
                &deb::Bundle {
                    libmpv_dir,
                    allow_gpl: options.allow_gpl,
                },
            )?
        }
    }
    let artifact = out.join(release.file_name(final_extension(channel, options.target, key)?));
    if !artifact.is_file() {
        bail!(
            "packaging finished but {} was not produced",
            artifact.display()
        );
    }
    match channel {
        Channel::Production => println!("update artifact: {}", artifact.display()),
        Channel::Development => println!("development installer: {}", artifact.display()),
    }
    Ok(())
}

/// Where the committed icon `file` of `channel` is.
pub(crate) fn icon_path(root: &Path, channel: Channel, file: &str) -> PathBuf {
    let variant = match channel {
        Channel::Production => crate::icons::Variant::Production,
        Channel::Development => crate::icons::Variant::Development,
    };
    crate::icons::generated_path(root, variant, file)
}

/// The committed icon `file` of `channel`; the packagers refuse to build without it.
pub(crate) fn channel_icon(root: &Path, channel: Channel, file: &str) -> Result<PathBuf> {
    let path = icon_path(root, channel, file);
    if !path.is_file() {
        bail!("{} is missing; run `cargo xtask icons`", path.display());
    }
    Ok(path)
}

/// `cargo xtask check-pe`: fails unless the Windows executable at `file` is a GUI program.
pub fn check_pe(file: &Path, gui: bool) -> Result<()> {
    if !gui {
        bail!("check-pe needs --gui: the only check is that the executable is a GUI program");
    }
    let bytes = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let subsystem = pe::pe_subsystem(&bytes)
        .with_context(|| format!("reading the PE header of {}", file.display()))?;
    if subsystem != 2 {
        bail!(
            "{} has PE subsystem {subsystem}, expected 2 (GUI): it would open a console window",
            file.display()
        );
    }
    println!("{}: PE subsystem 2 (GUI)", file.display());
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

#[cfg(test)]
mod arch_tests {
    use super::{absolute_out, effective_arch, platform_key, Target};

    #[test]
    fn a_relative_output_folder_becomes_absolute() {
        let out = absolute_out(std::path::Path::new("dist")).unwrap();
        assert!(out.is_absolute(), "{}", out.display());
        assert!(out.ends_with("dist"));
    }

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
