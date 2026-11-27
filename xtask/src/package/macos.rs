//! The macOS package: `Video All-in-One.app` with its libraries inside, an ad-hoc signature,
//! the zipped bundle (the update artifact) and a disk image.
//! FFmpeg and FFprobe are signed in `Contents/Resources/bin` and load `Contents/Frameworks`.

use super::license::{bundled_tools, check_bundle_license};
use super::macos_libs::{self, Closure};
use super::{package_file_name, production_icon, run_tool, APP_NAME, BINARY_NAME, IDENTIFIER};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The lowest macOS version the app is meant to support. The bundle raises it when a library
/// it contains was built for a newer system.
const MINIMUM_SYSTEM_VERSION: &str = "13.0";

/// The icon file in `Contents/Resources` without its `.icns` extension (`CFBundleIconFile`).
const ICON_NAME: &str = "AppIcon";

/// The `Contents/Info.plist` text of the production app. `minimum_system` is the lowest macOS
/// version every file in the bundle can run on.
pub(super) fn info_plist(version: &str, minimum_system: &str) -> String {
    let version = xml_escape(version);
    let entries = [
        ("CFBundleDevelopmentRegion", "en".to_string()),
        ("CFBundleExecutable", BINARY_NAME.to_string()),
        ("CFBundleIconFile", ICON_NAME.to_string()),
        ("CFBundleIdentifier", IDENTIFIER.to_string()),
        ("CFBundleInfoDictionaryVersion", "6.0".to_string()),
        ("CFBundleName", APP_NAME.to_string()),
        ("CFBundleDisplayName", APP_NAME.to_string()),
        ("CFBundlePackageType", "APPL".to_string()),
        ("CFBundleShortVersionString", version.clone()),
        ("CFBundleVersion", version),
        ("LSMinimumSystemVersion", xml_escape(minimum_system)),
        ("NSPrincipalClass", "NSApplication".to_string()),
    ];
    let mut text = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n",
    );
    for (key, value) in entries {
        text.push_str(&format!("\t<key>{key}</key>\n\t<string>{value}</string>\n"));
    }
    text.push_str("\t<key>NSHighResolutionCapable</key>\n\t<true/>\n</dict>\n</plist>\n");
    text
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Builds the app bundle, then the zip and the disk image next to each other in `out`.
pub(super) fn build(
    root: &Path,
    binary: &Path,
    out: &Path,
    version: &str,
    key: &str,
    allow_gpl: bool,
    libmpv_dir: Option<&Path>,
) -> Result<()> {
    // Release packages bundle only our own LGPL build; the Homebrew closure is GPL.
    if libmpv_dir.is_none() && !allow_gpl {
        bail!(
            "pass --libmpv-dir <folder of `cargo xtask libmpv build`>: the Homebrew libraries are GPL builds (use {} only for local tests)",
            super::license::ALLOW_GPL_FLAG
        );
    }
    let tool_source =
        libmpv_dir.context("--libmpv-dir is required for bundled FFmpeg and FFprobe")?;
    let tools = bundled_tools(&tool_source.join("bin"), false, allow_gpl)?;
    let stage = out.join("stage-macos");
    if stage.exists() {
        fs::remove_dir_all(&stage).with_context(|| format!("clearing {}", stage.display()))?;
    }
    let app = stage.join(format!("{APP_NAME}.app"));
    let macos = app.join("Contents/MacOS");
    let frameworks = app.join("Contents/Frameworks");
    let resources = app.join("Contents/Resources");
    for dir in [&macos, &frameworks, &resources] {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    if let Some(dir) = libmpv_dir {
        copy_licenses(&dir.join("LICENSES"), &resources.join("third-party"))?;
    }
    let bin = resources.join("bin");
    fs::create_dir_all(&bin)?;
    let mut installed_tools = Vec::new();
    for tool in &tools {
        let target = bin.join(tool.file_name().context("tool has a file name")?);
        fs::copy(tool, &target).with_context(|| format!("copying {}", tool.display()))?;
        make_writable_executable(&target)?;
        installed_tools.push(target);
    }
    let icon = production_icon(root, "icon.icns")?;
    fs::copy(&icon, resources.join(format!("{ICON_NAME}.icns")))
        .with_context(|| format!("copying {}", icon.display()))?;
    let executable = macos.join(BINARY_NAME);
    fs::copy(binary, &executable).with_context(|| format!("copying {}", binary.display()))?;
    make_writable_executable(&executable)?;

    let mut found = find_libraries(&executable, libmpv_dir)?;
    for tool in &installed_tools {
        let closure = find_libraries(tool, libmpv_dir)?;
        remove_absolute_rpaths(tool)?;
        run_tool(
            Command::new("install_name_tool").args(macos_libs::install_name_args(
                None,
                &closure.binary_deps,
                tool,
            )),
        )?;
        for library in closure.libraries {
            if let Some(existing) = found
                .libraries
                .iter()
                .find(|item| item.name == library.name)
            {
                if existing.source != library.source {
                    bail!("{} would be bundled from two files", library.name);
                }
            } else {
                found.libraries.push(library);
            }
        }
    }
    let sources: Vec<PathBuf> = found.libraries.iter().map(|l| l.source.clone()).collect();
    for warning in check_bundle_license(&sources, allow_gpl)? {
        println!("warning: {warning}");
    }
    for library in &found.libraries {
        let target = frameworks.join(&library.name);
        fs::copy(&library.source, &target)
            .with_context(|| format!("copying {}", library.source.display()))?;
        make_writable_executable(&target)?;
        let id = Some(library.name.as_str());
        run_tool(
            Command::new("install_name_tool").args(macos_libs::install_name_args(
                id,
                &library.deps,
                &target,
            )),
        )?;
    }
    run_tool(
        Command::new("install_name_tool").args(macos_libs::install_name_args(
            None,
            &found.binary_deps,
            &executable,
        )),
    )?;

    // Run paths that point at the build machine's Homebrew folders must not stay in the bundle.
    for file in found
        .libraries
        .iter()
        .map(|library| frameworks.join(&library.name))
        .chain([executable.clone()])
        .chain(installed_tools.iter().cloned())
    {
        remove_absolute_rpaths(&file)?;
    }
    // The bundle can only run where every file in it can run.
    let mut versions = Vec::new();
    for file in found
        .libraries
        .iter()
        .map(|library| frameworks.join(&library.name))
        .chain([executable.clone()])
        .chain(installed_tools.iter().cloned())
    {
        versions.extend(minimum_os_of(&file)?);
    }
    let minimum = macos_libs::highest_version(MINIMUM_SYSTEM_VERSION, &versions);
    if minimum != MINIMUM_SYSTEM_VERSION {
        println!("note: bundled files need macOS {minimum}, so the app is marked as macOS {minimum} or later");
    }
    fs::write(
        app.join("Contents/Info.plist"),
        info_plist(version, &minimum),
    )
    .context("writing Info.plist")?;
    // install_name_tool invalidates signatures, and arm64 refuses unsigned code.
    for library in &found.libraries {
        adhoc_sign(&frameworks.join(&library.name))?;
    }
    adhoc_sign(&executable)?;
    for tool in &installed_tools {
        adhoc_sign(tool)?;
    }
    adhoc_sign(&app)?;
    run_tool(
        Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(&app),
    )?;

    let zip = out.join(package_file_name(version, key, "zip"));
    let dmg = out.join(package_file_name(version, key, "dmg"));
    for old in [&zip, &dmg] {
        if old.exists() {
            fs::remove_file(old).with_context(|| format!("removing {}", old.display()))?;
        }
    }
    run_tool(
        Command::new("ditto")
            .args(["-c", "-k", "--keepParent"])
            .arg(&app)
            .arg(&zip),
    )?;
    make_dmg(&app, &stage, &dmg)?;
    for file in [&zip, &dmg] {
        println!(
            "wrote {} ({} bytes)",
            file.display(),
            fs::metadata(file)?.len()
        );
    }
    println!("bundled {} libraries", found.libraries.len());
    Ok(())
}

/// Copies the licence texts and source URLs of the libmpv build into the bundle; a package
/// without them must not be built.
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
        fs::copy(entry.path(), to.join(entry.file_name()))?;
        count += 1;
    }
    if count == 0 {
        bail!("{} is empty", from.display());
    }
    Ok(())
}

/// The libraries `executable` needs, from `otool -L` on each file. With `libmpv_dir` they come
/// only from that build; otherwise from wherever Homebrew put them (local testing).
fn find_libraries(executable: &Path, libmpv_dir: Option<&Path>) -> Result<Closure> {
    let mut list = |path: &Path, _library: bool| -> Result<String> {
        let output = Command::new("otool")
            .arg("-L")
            .arg(path)
            .output()
            .context("starting otool")?;
        if !output.status.success() {
            bail!("otool failed for {}", path.display());
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    };
    match libmpv_dir {
        Some(dir) => {
            let lib = dir.join("lib");
            macos_libs::closure(executable, &mut list, &|dep, referrer| {
                resolve_in_build(dep, referrer, &lib)
            })
        }
        None => macos_libs::closure(executable, &mut list, &macos_libs::resolve_on_disk),
    }
}

/// The file behind `dep` inside our libmpv build `lib`. The executable (no `referrer_dir`) and
/// the libraries refer to each other as `@rpath/<file>` or `@loader_path/<file>`; anything else
/// would point outside the build and is refused.
fn resolve_in_build(dep: &str, referrer_dir: &Path, lib: &Path) -> Result<PathBuf> {
    let candidate = if let Some(name) = dep.strip_prefix("@rpath/") {
        lib.join(name)
    } else if let Some(name) = dep.strip_prefix("@loader_path/") {
        referrer_dir.join(name)
    } else {
        bail!("{dep} points outside the libmpv build ({})", lib.display());
    };
    candidate
        .canonicalize()
        .with_context(|| format!("{dep} is not in {}", lib.display()))
}

fn minimum_os_of(file: &Path) -> Result<Option<String>> {
    let output = Command::new("otool")
        .arg("-l")
        .arg(file)
        .output()
        .context("starting otool")?;
    if !output.status.success() {
        bail!("otool -l failed for {}", file.display());
    }
    Ok(macos_libs::minimum_os(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn remove_absolute_rpaths(file: &Path) -> Result<()> {
    let output = Command::new("otool")
        .arg("-l")
        .arg(file)
        .output()
        .context("starting otool")?;
    if !output.status.success() {
        bail!("otool -l failed for {}", file.display());
    }
    for rpath in macos_libs::absolute_rpaths(&String::from_utf8_lossy(&output.stdout)) {
        run_tool(
            Command::new("install_name_tool")
                .args(["-delete_rpath", &rpath])
                .arg(file),
        )?;
    }
    Ok(())
}

fn adhoc_sign(path: &Path) -> Result<()> {
    run_tool(
        Command::new("codesign")
            .args(["--force", "--sign", "-"])
            .arg(path),
    )
}

/// A disk image with the app and a shortcut to /Applications.
fn make_dmg(app: &Path, stage: &Path, dmg: &Path) -> Result<()> {
    let folder: PathBuf = stage.join("dmg");
    fs::create_dir_all(&folder)?;
    let copy = folder.join(format!("{APP_NAME}.app"));
    run_tool(Command::new("ditto").arg(app).arg(&copy))?;
    run_tool(
        Command::new("ln")
            .args(["-s", "/Applications"])
            .arg(folder.join("Applications")),
    )?;
    run_tool(
        Command::new("hdiutil")
            .args([
                "create",
                "-volname",
                APP_NAME,
                "-ov",
                "-format",
                "UDZO",
                "-srcfolder",
            ])
            .arg(&folder)
            .arg(dmg),
    )
}

#[cfg(unix)]
fn make_writable_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("setting permissions of {}", path.display()))
}

#[cfg(not(unix))]
fn make_writable_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "macos_tests.rs"]
mod tests;
