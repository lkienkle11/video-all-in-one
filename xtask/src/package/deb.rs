//! The Ubuntu package: a `.deb` staged as a plain directory tree and built with `dpkg-deb`.

use super::{package_file_name, production_icon, run_tool, APP_NAME, BINARY_NAME};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Where the icon is installed, relative to the package root.
pub(super) const ICON_PATH: &str = "usr/share/icons/hicolor/512x512/apps/video-all-in-one.png";

/// The `DEBIAN/control` text. `installed_kib` is the size of the staged files in KiB.
pub(super) fn control_file(version: &str, installed_kib: u64) -> String {
    // libmpv2 is the Ubuntu 24.04 package of libmpv.so.2 (mpv client API 2), the library the
    // release build links on that runner; Ubuntu 22.04 only has libmpv1 (API 1), which the app
    // refuses at run time. ffmpeg is only recommended: the app tells the user when it is missing.
    [
        format!("Package: {BINARY_NAME}"),
        format!("Version: {version}"),
        "Section: video".to_string(),
        "Priority: optional".to_string(),
        "Architecture: amd64".to_string(),
        format!("Installed-Size: {installed_kib}"),
        "Depends: libmpv2, libc6 (>= 2.39)".to_string(),
        "Recommends: ffmpeg".to_string(),
        format!("Maintainer: {APP_NAME} project <noreply@example.invalid>"),
        format!("Description: {APP_NAME}, a desktop app to cut and export videos"),
    ]
    .join("\n")
        + "\n"
}

/// The `/usr/share/applications` entry.
pub(super) fn desktop_entry() -> String {
    [
        "[Desktop Entry]".to_string(),
        "Type=Application".to_string(),
        format!("Name={APP_NAME}"),
        format!("Exec=/usr/bin/{BINARY_NAME}"),
        "Terminal=false".to_string(),
        format!("Icon={BINARY_NAME}"),
        "Categories=AudioVideo;Video;".to_string(),
    ]
    .join("\n")
        + "\n"
}

/// Stages the package tree, builds it with `dpkg-deb` and leaves the `.deb` in `out`.
pub(super) fn build(
    root: &Path,
    binary: &Path,
    out: &Path,
    version: &str,
    key: &str,
) -> Result<()> {
    let stage = out.join("stage-deb");
    if stage.exists() {
        fs::remove_dir_all(&stage).with_context(|| format!("clearing {}", stage.display()))?;
    }
    let tree = stage.join(format!("{BINARY_NAME}_{version}_amd64"));
    let bin_dir = tree.join("usr/bin");
    let apps_dir = tree.join("usr/share/applications");
    let control_dir = tree.join("DEBIAN");
    for dir in [&bin_dir, &apps_dir, &control_dir] {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let installed = bin_dir.join(BINARY_NAME);
    fs::copy(binary, &installed).with_context(|| format!("copying {}", binary.display()))?;
    set_mode(&installed, 0o755)?;
    let desktop = apps_dir.join(format!("{BINARY_NAME}.desktop"));
    fs::write(&desktop, desktop_entry())?;
    set_mode(&desktop, 0o644)?;
    let icon = tree.join(ICON_PATH);
    let icon_dir = icon.parent().context("the icon path has a folder")?;
    fs::create_dir_all(icon_dir).with_context(|| format!("creating {}", icon_dir.display()))?;
    let icon_source = production_icon(root, "icon.png")?;
    fs::copy(&icon_source, &icon).with_context(|| format!("copying {}", icon_source.display()))?;
    set_mode(&icon, 0o644)?;
    let kib = fs::metadata(&installed)?.len().div_ceil(1024);
    let control = control_dir.join("control");
    fs::write(&control, control_file(version, kib))?;
    set_mode(&control, 0o644)?;
    let share = tree.join("usr/share");
    let mut dirs = vec![
        tree.clone(),
        tree.join("usr"),
        bin_dir,
        apps_dir,
        control_dir,
    ];
    dirs.extend(
        icon_dir
            .ancestors()
            .take_while(|dir| *dir != tree.join("usr"))
            .map(Path::to_path_buf),
    );
    dirs.push(share);
    for dir in &dirs {
        set_mode(dir, 0o755)?;
    }
    let deb = out.join(package_file_name(version, key, "deb"));
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
    use super::{control_file, desktop_entry, ICON_PATH};

    fn field<'a>(text: &'a str, name: &str) -> Option<&'a str> {
        text.lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix(": "))
    }

    #[test]
    fn control_file_depends_on_the_ubuntu_22_04_libmpv_and_recommends_ffmpeg() {
        let control = control_file("0.1.0", 30000);
        assert_eq!(field(&control, "Package"), Some("video-all-in-one"));
        assert_eq!(field(&control, "Version"), Some("0.1.0"));
        assert_eq!(field(&control, "Architecture"), Some("amd64"));
        assert_eq!(field(&control, "Installed-Size"), Some("30000"));
        let depends = field(&control, "Depends").unwrap();
        assert!(depends.split(", ").any(|d| d == "libmpv2"), "{depends}");
        assert_eq!(field(&control, "Recommends"), Some("ffmpeg"));
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
    fn desktop_entry_starts_the_executable_from_usr_bin() {
        let entry = desktop_entry();
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
            ICON_PATH,
            "usr/share/icons/hicolor/512x512/apps/video-all-in-one.png"
        );
    }
}
