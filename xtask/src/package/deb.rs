//! The Ubuntu package: a `.deb` staged as a plain directory tree and built with `dpkg-deb`.

use super::{package_file_name, run_tool, APP_NAME, BINARY_NAME};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

/// The `DEBIAN/control` text. `installed_kib` is the size of the staged files in KiB.
pub(super) fn control_file(version: &str, installed_kib: u64) -> String {
    // libmpv1 is the Ubuntu 22.04 package of libmpv.so.1, the library the release build
    // links on that runner. ffmpeg is only recommended: the app tells the user when it is missing.
    [
        format!("Package: {BINARY_NAME}"),
        format!("Version: {version}"),
        "Section: video".to_string(),
        "Priority: optional".to_string(),
        "Architecture: amd64".to_string(),
        format!("Installed-Size: {installed_kib}"),
        "Depends: libmpv1, libc6 (>= 2.35)".to_string(),
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
        "Categories=AudioVideo;Video;".to_string(),
    ]
    .join("\n")
        + "\n"
}

/// Stages the package tree, builds it with `dpkg-deb` and leaves the `.deb` in `out`.
pub(super) fn build(binary: &Path, out: &Path, version: &str, key: &str) -> Result<()> {
    let stage = out.join("stage-deb");
    if stage.exists() {
        fs::remove_dir_all(&stage).with_context(|| format!("clearing {}", stage.display()))?;
    }
    let root = stage.join(format!("{BINARY_NAME}_{version}_amd64"));
    let bin_dir = root.join("usr/bin");
    let apps_dir = root.join("usr/share/applications");
    let control_dir = root.join("DEBIAN");
    for dir in [&bin_dir, &apps_dir, &control_dir] {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let installed = bin_dir.join(BINARY_NAME);
    fs::copy(binary, &installed).with_context(|| format!("copying {}", binary.display()))?;
    set_mode(&installed, 0o755)?;
    let desktop = apps_dir.join(format!("{BINARY_NAME}.desktop"));
    fs::write(&desktop, desktop_entry())?;
    set_mode(&desktop, 0o644)?;
    let kib = fs::metadata(&installed)?.len().div_ceil(1024);
    let control = control_dir.join("control");
    fs::write(&control, control_file(version, kib))?;
    set_mode(&control, 0o644)?;
    for dir in [
        &root,
        &root.join("usr"),
        &bin_dir,
        &root.join("usr/share"),
        &apps_dir,
        &control_dir,
    ] {
        set_mode(dir, 0o755)?;
    }
    let deb = out.join(package_file_name(version, key, "deb"));
    if deb.exists() {
        fs::remove_file(&deb)?;
    }
    run_tool(
        Command::new("dpkg-deb")
            .args(["--build", "--root-owner-group"])
            .arg(&root)
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
    use super::{control_file, desktop_entry};

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
        assert!(depends.split(", ").any(|d| d == "libmpv1"), "{depends}");
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
            "Categories=AudioVideo;Video;",
        ] {
            assert!(entry.lines().any(|l| l == line), "missing {line}\n{entry}");
        }
    }
}
