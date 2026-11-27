//! The Ubuntu package: a `.deb` staged as a plain directory tree and built with `dpkg-deb`.
//! It carries its own LGPL libmpv and FFmpeg (built by `cargo xtask libmpv build`) in
//! `/usr/lib/video-all-in-one`, so one package runs on Ubuntu 22.04 and later.
//! FFmpeg and FFprobe live in its `bin` subfolder with `$ORIGIN/..` as their run path.

use super::elf;
use super::license::{bundled_tools, check_bundle_license};
use super::{channel_icon, run_tool, stage_name, Release};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use vaio_platform::channel::Channel;

/// Where the icon is installed, relative to the package root.
pub(super) fn icon_path(channel: Channel) -> String {
    format!(
        "usr/share/icons/hicolor/512x512/apps/{}.png",
        channel.binary_name()
    )
}

/// Where the bundled libraries are installed, relative to the package root.
pub(super) fn lib_dir(channel: Channel) -> String {
    format!("usr/lib/{}", channel.binary_name())
}

/// Where the licence texts of the bundled libraries are installed, relative to the package root.
pub(super) fn licenses_dir(channel: Channel) -> String {
    format!("usr/share/doc/{}/third-party", channel.binary_name())
}

/// The run path the executable must have been linked with (`$ORIGIN` is the folder of the
/// executable, `/usr/bin`). The release workflow sets it through `RUSTFLAGS`.
pub(super) fn run_path(channel: Channel) -> String {
    format!("$ORIGIN/../lib/{}", channel.binary_name())
}

/// The `DEBIAN/control` text. `installed_kib` is the size of the staged files in KiB and
/// `depends` the system packages the executable and the bundled libraries need
/// and tools need (see `elf::system_depends`).
pub(super) fn control_file(
    channel: Channel,
    version: &str,
    installed_kib: u64,
    depends: &[String],
) -> String {
    let app_name = channel.app_name();
    [
        format!("Package: {}", channel.binary_name()),
        format!("Version: {version}"),
        "Section: video".to_string(),
        "Priority: optional".to_string(),
        "Architecture: amd64".to_string(),
        format!("Installed-Size: {installed_kib}"),
        format!("Depends: {}", depends.join(", ")),
        format!("Maintainer: {app_name} project <noreply@example.invalid>"),
        format!("Description: {app_name}, a desktop app to cut and export videos"),
    ]
    .join("\n")
        + "\n"
}

/// The `/usr/share/applications` entry.
pub(super) fn desktop_entry(channel: Channel) -> String {
    let (app_name, binary_name) = (channel.app_name(), channel.binary_name());
    [
        "[Desktop Entry]".to_string(),
        "Type=Application".to_string(),
        format!("Name={app_name}"),
        format!("Exec=/usr/bin/{binary_name}"),
        "Terminal=false".to_string(),
        format!("Icon={binary_name}"),
        "Categories=AudioVideo;Video;".to_string(),
    ]
    .join("\n")
        + "\n"
}

/// What `build` needs besides the executable.
pub(super) struct Bundle<'a> {
    /// The folder of our own libmpv build (`lib`, `LICENSES`, ...).
    pub libmpv_dir: &'a Path,
    pub allow_gpl: bool,
}

/// Stages the package tree, builds it with `dpkg-deb` and leaves the `.deb` in `out`.
pub(super) fn build(
    root: &Path,
    binary: &Path,
    out: &Path,
    release: &Release,
    bundle: &Bundle,
) -> Result<()> {
    let Release {
        channel, version, ..
    } = *release;
    let binary_name = channel.binary_name();
    let run_path = run_path(channel);
    let tools = bundled_tools(&bundle.libmpv_dir.join("bin"), false, bundle.allow_gpl)?;
    let stage = out.join(stage_name(channel, "deb"));
    if stage.exists() {
        fs::remove_dir_all(&stage).with_context(|| format!("clearing {}", stage.display()))?;
    }
    let tree = stage.join(format!("{binary_name}_{version}_amd64"));
    let bin_dir = tree.join("usr/bin");
    let apps_dir = tree.join("usr/share/applications");
    let control_dir = tree.join("DEBIAN");
    let lib_dir = tree.join(lib_dir(channel));
    for dir in [&bin_dir, &apps_dir, &control_dir, &lib_dir] {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let tool_dir = lib_dir.join("bin");
    fs::create_dir_all(&tool_dir)?;
    let mut installed_tools = Vec::new();
    let mut kib_tools = 0;
    for source in &tools {
        let target = tool_dir.join(source.file_name().context("tool has a file name")?);
        fs::copy(source, &target).with_context(|| format!("copying {}", source.display()))?;
        set_mode(&target, 0o755)?;
        kib_tools += fs::metadata(&target)?.len().div_ceil(1024);
        installed_tools.push(target);
    }
    let installed = bin_dir.join(binary_name);
    fs::copy(binary, &installed).with_context(|| format!("copying {}", binary.display()))?;
    set_mode(&installed, 0o755)?;
    let executable_dynamic = readelf_dynamic(&installed)?;
    if !elf::run_paths(&executable_dynamic).contains(&run_path) {
        bail!(
            "{} was not linked with the run path {run_path}; build it with RUSTFLAGS containing `-C link-arg=-Wl,-rpath,{run_path}` (see docs/RELEASING.md)",
            binary.display()
        );
    }
    let (bundled, kib_libs) = stage_libraries(bundle, &lib_dir)?;
    let mut needed = elf::needed(&executable_dynamic);
    for tool in &installed_tools {
        run_tool(
            Command::new("patchelf")
                .args(["--set-rpath", "$ORIGIN/.."])
                .arg(tool),
        )?;
        let dynamic = readelf_dynamic(tool)?;
        if elf::run_paths(&dynamic) != ["$ORIGIN/.."] {
            bail!("{} must have the run path $ORIGIN/.. only", tool.display());
        }
        needed.extend(elf::needed(&dynamic));
    }
    let mut real_libraries = Vec::new();
    for name in &bundled {
        let path = lib_dir.join(name);
        if !fs::symlink_metadata(&path)?.file_type().is_symlink() {
            needed.extend(elf::needed(&readelf_dynamic(&path)?));
            real_libraries.push(path);
        }
    }
    for warning in check_bundle_license(&real_libraries, bundle.allow_gpl)? {
        println!("warning: {warning}");
    }
    let depends = elf::system_depends(&needed, &bundled)?;
    println!("Depends: {}", depends.join(", "));
    let licenses = tree.join(licenses_dir(channel));
    copy_licenses(&bundle.libmpv_dir.join("LICENSES"), &licenses)?;
    let desktop = apps_dir.join(format!("{binary_name}.desktop"));
    fs::write(&desktop, desktop_entry(channel))?;
    set_mode(&desktop, 0o644)?;
    let icon = tree.join(icon_path(channel));
    let icon_dir = icon.parent().context("the icon path has a folder")?;
    fs::create_dir_all(icon_dir).with_context(|| format!("creating {}", icon_dir.display()))?;
    let icon_source = channel_icon(root, channel, "icon.png")?;
    fs::copy(&icon_source, &icon).with_context(|| format!("copying {}", icon_source.display()))?;
    set_mode(&icon, 0o644)?;
    let kib = (fs::metadata(&installed)?.len().div_ceil(1024)) + kib_libs + kib_tools;
    let control = control_dir.join("control");
    fs::write(&control, control_file(channel, version, kib, &depends))?;
    set_mode(&control, 0o644)?;
    set_directory_modes(&tree)?;
    let deb = out.join(release.file_name("deb"));
    if deb.exists() {
        fs::remove_file(&deb)?;
    }
    run_tool(
        Command::new("dpkg-deb")
            .args(["--build", "--root-owner-group"])
            .arg(&tree)
            .arg(&deb),
    )?;
    println!("wrote {}", deb.display());
    Ok(())
}

/// The text of `readelf -d` for `file`.
fn readelf_dynamic(file: &Path) -> Result<String> {
    let output = Command::new("readelf")
        .arg("-d")
        .arg(file)
        .output()
        .context("starting readelf")?;
    if !output.status.success() {
        bail!("readelf failed for {}", file.display());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Copies the shared libraries (and their version symlinks) of our libmpv build into `lib_dir`.
/// Returns their file names and their size in KiB.
fn stage_libraries(bundle: &Bundle, lib_dir: &Path) -> Result<(Vec<String>, u64)> {
    let source = bundle.libmpv_dir.join("lib");
    let mut names = Vec::new();
    let mut kib = 0;
    let entries = fs::read_dir(&source).with_context(|| {
        format!(
            "reading {} (is --libmpv-dir a libmpv build?)",
            source.display()
        )
    })?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.contains(".so") {
            continue;
        }
        let to = lib_dir.join(&name);
        if entry.file_type()?.is_symlink() {
            copy_symlink(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to).with_context(|| format!("copying {name}"))?;
            set_mode(&to, 0o644)?;
            kib += fs::metadata(&to)?.len().div_ceil(1024);
        }
        names.push(name);
    }
    if !names.iter().any(|n| n.starts_with("libmpv.so")) {
        bail!("{} has no libmpv.so", source.display());
    }
    names.sort();
    Ok((names, kib))
}

/// Copies the licence files of the libmpv build; a package without them must not be built.
fn copy_licenses(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("creating {}", to.display()))?;
    let entries = fs::read_dir(from).with_context(|| {
        format!(
            "reading {}: the libmpv build has no LICENSES",
            from.display()
        )
    })?;
    let mut count = 0;
    for entry in entries {
        let entry = entry?;
        let target = to.join(entry.file_name());
        fs::copy(entry.path(), &target)?;
        set_mode(&target, 0o644)?;
        count += 1;
    }
    if count == 0 {
        bail!("{} is empty", from.display());
    }
    Ok(())
}

/// Every folder of the package is `rwxr-xr-x`.
fn set_directory_modes(tree: &Path) -> Result<()> {
    let mut pending: Vec<PathBuf> = vec![tree.to_path_buf()];
    while let Some(dir) = pending.pop() {
        set_mode(&dir, 0o755)?;
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn copy_symlink(from: &Path, to: &Path) -> Result<()> {
    let target = fs::read_link(from).with_context(|| format!("reading {}", from.display()))?;
    std::os::unix::fs::symlink(&target, to).with_context(|| format!("linking {}", to.display()))
}

#[cfg(not(unix))]
fn copy_symlink(from: &Path, _to: &Path) -> Result<()> {
    bail!(
        "{} is a symlink; the Ubuntu package is built on Linux only",
        from.display()
    )
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("setting permissions of {}", path.display()))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{control_file, desktop_entry, icon_path, lib_dir, licenses_dir, run_path};
    use vaio_platform::channel::Channel;

    const PRODUCTION: Channel = Channel::Production;

    fn field<'a>(text: &'a str, name: &str) -> Option<&'a str> {
        text.lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix(": "))
    }

    fn depends() -> Vec<String> {
        vec!["libc6 (>= 2.35)".to_string(), "libasound2".to_string()]
    }

    #[test]
    fn control_file_lists_only_what_the_bundled_libmpv_needs_from_the_system() {
        let control = control_file(PRODUCTION, "0.1.0", 30000, &depends());
        assert_eq!(field(&control, "Package"), Some("video-all-in-one"));
        assert_eq!(field(&control, "Version"), Some("0.1.0"));
        assert_eq!(field(&control, "Architecture"), Some("amd64"));
        assert_eq!(field(&control, "Installed-Size"), Some("30000"));
        assert_eq!(
            field(&control, "Depends"),
            Some("libc6 (>= 2.35), libasound2")
        );
        assert!(
            !control.contains("libmpv"),
            "libmpv is bundled, not a dependency"
        );
        assert_eq!(field(&control, "Recommends"), None);
        assert!(
            field(&control, "Maintainer").is_some(),
            "dpkg wants a maintainer"
        );
        assert!(
            control.ends_with('\n'),
            "a control file ends with a newline"
        );
        assert!(!control.contains("\n\n"), "no blank line inside the stanza");
    }

    #[test]
    fn bundled_files_live_where_the_executable_run_path_looks() {
        assert_eq!(lib_dir(PRODUCTION), "usr/lib/video-all-in-one");
        // The executable is /usr/bin/video-all-in-one, so $ORIGIN/.. is /usr.
        assert_eq!(run_path(PRODUCTION), "$ORIGIN/../lib/video-all-in-one");
        assert_eq!(
            licenses_dir(PRODUCTION),
            "usr/share/doc/video-all-in-one/third-party"
        );
    }

    #[test]
    fn desktop_entry_starts_the_executable_from_usr_bin() {
        let entry = desktop_entry(PRODUCTION);
        assert!(entry.starts_with("[Desktop Entry]\n"));
        for line in [
            "Type=Application",
            "Name=Video All-in-One",
            "Exec=/usr/bin/video-all-in-one",
            "Terminal=false",
            "Icon=video-all-in-one",
            "Categories=AudioVideo;Video;",
        ] {
            assert!(entry.lines().any(|l| l == line), "missing {line}\n{entry}");
        }
    }

    #[test]
    fn icon_is_installed_where_the_desktop_entry_finds_it_by_name() {
        assert_eq!(
            icon_path(PRODUCTION),
            "usr/share/icons/hicolor/512x512/apps/video-all-in-one.png"
        );
    }
}
