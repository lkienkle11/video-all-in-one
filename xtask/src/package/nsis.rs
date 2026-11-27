//! The Windows package: an NSIS installer script generated from a template and built with
//! `makensis`. The installer is per user (no administrator rights) and installs silently
//! with `/S`, which is how the app's updater runs it.
//! FFmpeg and FFprobe live in `bin` with DLL copies for the Windows loader; licence texts
//! and source URLs from the build's `LICENSES` folder live in `third-party`.

use super::license::{bundled_tools, check_bundle_license};
use super::{channel_icon, run_tool, stage_name, Release};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;
use vaio_platform::channel::Channel;

/// Quotes `text` for an NSIS string: `$` and `"` are escaped, line breaks are refused.
pub(super) fn quote(text: &str) -> Result<String> {
    if text.contains(['\n', '\r']) {
        bail!("a line break cannot be written into the installer script");
    }
    Ok(format!(
        "\"{}\"",
        text.replace('$', "$$").replace('"', "$\\\"")
    ))
}

/// File names that are installed must be plain, so no name can change the script around it.
fn check_file_name(name: &str) -> Result<()> {
    let plain = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if !plain {
        bail!("`{name}` is not a plain file name for the installer");
    }
    Ok(())
}

/// The installer script. `files` are the names of the files in `source_dir` to install, the
/// executable first; `output` is the installer path. Only the dynamic values go through
/// [`quote`]; the rest is fixed text that uses NSIS variables on purpose.
pub(super) fn installer_script(
    channel: Channel,
    version: &str,
    output: &Path,
    source_dir: &Path,
    files: &[String],
    icon: &Path,
) -> Result<String> {
    for file in files {
        check_file_name(file.strip_prefix("third-party\\").unwrap_or(file))?;
    }
    let (app_name, identifier) = (channel.app_name(), channel.identifier());
    let exe = format!("$INSTDIR\\{}.exe", channel.binary_name());
    let shortcut = format!("$SMPROGRAMS\\{app_name}.lnk");
    let key = format!("\"Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{identifier}\"");
    let mut lines: Vec<String> = vec![
        "Unicode true".into(),
        "RequestExecutionLevel user".into(),
        format!("Name \"{app_name}\""),
        format!("OutFile {}", quote(&output.display().to_string())?),
        format!("InstallDir \"$LOCALAPPDATA\\Programs\\{app_name}\""),
        "SetCompressor /SOLID lzma".into(),
        format!("Icon {}", quote(&icon.display().to_string())?),
        format!("UninstallIcon {}", quote(&icon.display().to_string())?),
        "Page directory".into(),
        "Page instfiles".into(),
        "UninstPage uninstConfirm".into(),
        "UninstPage instfiles".into(),
        String::new(),
        "Section \"Install\"".into(),
        "  SetOutPath \"$INSTDIR\"".into(),
    ];
    for file in files {
        let destination = if file.starts_with("third-party\\") {
            "$INSTDIR\\third-party"
        } else {
            "$INSTDIR"
        };
        lines.push(format!("  SetOutPath \"{destination}\""));
        let path = format!("{}\\{file}", source_dir.display());
        lines.push(format!("  File {}", quote(&path)?));
    }
    lines.push("  SetOutPath \"$INSTDIR\\bin\"".into());
    for file in files
        .iter()
        .filter(|file| file.to_ascii_lowercase().ends_with(".dll"))
    {
        lines.push(format!(
            "  File {}",
            quote(&format!("{}\\{file}", source_dir.display()))?
        ));
    }
    for name in ["ffmpeg.exe", "ffprobe.exe"] {
        lines.push(format!(
            "  File {}",
            quote(&format!("{}\\bin\\{name}", source_dir.display()))?
        ));
    }
    lines.push("  SetOutPath \"$INSTDIR\"".into());
    lines.extend([
        "  WriteUninstaller \"$INSTDIR\\uninstall.exe\"".to_string(),
        format!("  CreateShortcut \"{shortcut}\" \"{exe}\""),
        format!("  WriteRegStr HKCU {key} \"DisplayName\" \"{app_name}\""),
        format!("  WriteRegStr HKCU {key} \"DisplayVersion\" {}", quote(version)?),
        format!("  WriteRegStr HKCU {key} \"InstallLocation\" \"$INSTDIR\""),
        format!("  WriteRegStr HKCU {key} \"UninstallString\" \"$\\\"$INSTDIR\\uninstall.exe$\\\"\""),
        format!("  WriteRegStr HKCU {key} \"QuietUninstallString\" \"$\\\"$INSTDIR\\uninstall.exe$\\\" /S\""),
        format!("  WriteRegDWORD HKCU {key} \"NoModify\" 1"),
        format!("  WriteRegDWORD HKCU {key} \"NoRepair\" 1"),
        "SectionEnd".to_string(),
        String::new(),
        "Section \"Uninstall\"".to_string(),
    ]);
    for file in files {
        lines.push(format!("  Delete \"$INSTDIR\\{file}\""));
    }
    for file in files
        .iter()
        .filter(|file| file.to_ascii_lowercase().ends_with(".dll"))
    {
        lines.push(format!("  Delete \"$INSTDIR\\bin\\{file}\""));
    }
    for name in ["ffmpeg.exe", "ffprobe.exe"] {
        lines.push(format!("  Delete \"$INSTDIR\\bin\\{name}\""));
    }
    lines.push("  RMDir \"$INSTDIR\\bin\"".into());
    lines.push("  RMDir \"$INSTDIR\\third-party\"".into());
    lines.extend([
        "  Delete \"$INSTDIR\\uninstall.exe\"".to_string(),
        format!("  Delete \"{shortcut}\""),
        format!("  DeleteRegKey HKCU {key}"),
        "  RMDir \"$INSTDIR\"".to_string(),
        "SectionEnd".to_string(),
        String::new(),
        "Function .onInstSuccess".to_string(),
        "  IfSilent 0 done".to_string(),
        format!("  Exec '\"{exe}\"'"),
        "  done:".to_string(),
        "FunctionEnd".to_string(),
    ]);
    Ok(lines.join("\n") + "\n")
}

pub(super) fn build(
    root: &Path,
    binary: &Path,
    out: &Path,
    release: &Release,
    libs: Option<&Path>,
    allow_gpl: bool,
) -> Result<()> {
    let Release {
        channel, version, ..
    } = *release;
    let Some(libs) = libs else {
        bail!("--libs is required on Windows: the folder with libmpv-2.dll and the DLLs it needs");
    };
    let tools = bundled_tools(libs, true, allow_gpl)?;
    let stage = out.join(stage_name(channel, "windows"));
    if stage.exists() {
        fs::remove_dir_all(&stage).with_context(|| format!("clearing {}", stage.display()))?;
    }
    fs::create_dir_all(&stage)?;
    fs::create_dir_all(stage.join("bin"))?;
    for tool in &tools {
        fs::copy(
            tool,
            stage
                .join("bin")
                .join(tool.file_name().context("tool has a file name")?),
        )
        .with_context(|| format!("copying {}", tool.display()))?;
    }
    let exe_name = format!("{}.exe", channel.binary_name());
    fs::copy(binary, stage.join(&exe_name))
        .with_context(|| format!("copying {}", binary.display()))?;
    let mut files = vec![exe_name];
    let mut dlls = Vec::new();
    for entry in fs::read_dir(libs).with_context(|| format!("reading {}", libs.display()))? {
        let path = entry?.path();
        let is_dll = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"));
        if path.is_file() && is_dll {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            fs::copy(&path, stage.join(&name))
                .with_context(|| format!("copying {}", path.display()))?;
            files.push(name);
            dlls.push(path);
        }
    }
    for warning in check_bundle_license(&dlls, allow_gpl)? {
        println!("warning: {warning}");
    }
    if !files
        .iter()
        .any(|name| name.eq_ignore_ascii_case("libmpv-2.dll"))
    {
        bail!("{} has no libmpv-2.dll", libs.display());
    }
    fs::create_dir_all(stage.join("third-party"))?;
    let mut licenses = Vec::new();
    for entry in fs::read_dir(libs.join("LICENSES"))
        .context("the --libs folder must contain LICENSES with licence texts and source URLs")?
    {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        check_file_name(&name)?;
        fs::copy(entry.path(), stage.join("third-party").join(&name))?;
        licenses.push(format!("third-party\\{name}"));
    }
    if licenses.is_empty() {
        bail!("the --libs LICENSES folder is empty");
    }
    licenses.sort();
    files.extend(licenses);
    let installer = out.join(release.file_name("exe"));
    if installer.exists() {
        fs::remove_file(&installer)?;
    }
    // The icon sits next to the script, outside the folder of installed files.
    let icon = out.join("icon.ico");
    let icon_source = channel_icon(root, channel, "icon.ico")?;
    fs::copy(&icon_source, &icon).with_context(|| format!("copying {}", icon_source.display()))?;
    let script = installer_script(channel, version, &installer, &stage, &files, &icon)?;
    let script_path = out.join("installer.nsi");
    fs::write(&script_path, script)
        .with_context(|| format!("writing {}", script_path.display()))?;
    run_tool(Command::new("makensis").arg("-V2").arg(&script_path))?;
    println!("wrote {} with {} files", installer.display(), files.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{installer_script, quote};
    use std::path::Path;
    use vaio_platform::channel::Channel;

    fn script() -> String {
        installer_script(
            Channel::Production,
            "0.1.0",
            Path::new("C:\\out\\video-all-in-one-0.1.0-windows-x86_64.exe"),
            Path::new("C:\\out\\stage"),
            &[
                "video-all-in-one.exe".to_string(),
                "libmpv-2.dll".to_string(),
            ],
            Path::new("C:\\out\\icon.ico"),
        )
        .unwrap()
    }

    #[test]
    fn script_uses_the_icon_for_the_installer_and_the_uninstaller() {
        let script = script();
        assert!(script.contains("Icon \"C:\\out\\icon.ico\"\n"));
        assert!(script.contains("UninstallIcon \"C:\\out\\icon.ico\"\n"));
        assert!(
            !script.contains("File \"C:\\out\\icon.ico\""),
            "the icon is not installed as a file"
        );
    }

    #[test]
    fn quote_escapes_dollar_and_quote_and_refuses_line_breaks() {
        assert_eq!(quote("a $b \"c\"").unwrap(), "\"a $$b $\\\"c$\\\"\"");
        assert!(quote("a\nb").is_err());
        assert!(quote("a\rb").is_err());
    }

    #[test]
    fn script_installs_per_user_and_names_the_installer() {
        let script = script();
        assert!(script.contains("RequestExecutionLevel user"));
        assert!(script.contains("InstallDir \"$LOCALAPPDATA\\Programs\\Video All-in-One\""));
        assert!(script.contains("OutFile \"C:\\out\\video-all-in-one-0.1.0-windows-x86_64.exe\""));
        assert!(script.contains("Unicode true"));
    }

    #[test]
    fn script_copies_every_file_and_deletes_exactly_those_files_on_uninstall() {
        let script = script();
        for file in ["video-all-in-one.exe", "libmpv-2.dll"] {
            assert!(
                script.contains(&format!("File \"C:\\out\\stage\\{file}\"")),
                "{file}"
            );
            assert!(
                script.contains(&format!("Delete \"$INSTDIR\\{file}\"")),
                "{file}"
            );
        }
        assert!(
            !script.contains("RMDir /r"),
            "the uninstaller must never delete a folder tree"
        );
        assert!(script.contains("RMDir \"$INSTDIR\""));
        assert!(script.contains("WriteUninstaller \"$INSTDIR\\uninstall.exe\""));
        assert!(script.contains("Section \"Uninstall\""));
    }

    #[test]
    fn script_adds_a_start_menu_shortcut_and_an_uninstall_entry_for_the_current_user() {
        let script = script();
        assert!(script.contains("CreateShortcut \"$SMPROGRAMS\\Video All-in-One.lnk\" \"$INSTDIR\\video-all-in-one.exe\""));
        assert!(script.contains("Delete \"$SMPROGRAMS\\Video All-in-One.lnk\""));
        let key = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\app.videoallinone";
        assert!(script.contains(&format!(
            "WriteRegStr HKCU \"{key}\" \"DisplayVersion\" \"0.1.0\""
        )));
        assert!(script.contains(&format!("DeleteRegKey HKCU \"{key}\"")));
        assert!(
            !script.contains("HKLM"),
            "a per-user install never writes machine-wide keys"
        );
    }

    #[test]
    fn silent_install_starts_the_app_again_after_an_update() {
        let script = script();
        assert!(script.contains("Function .onInstSuccess"));
        assert!(script.contains("IfSilent"));
        assert!(script.contains("Exec '\"$INSTDIR\\video-all-in-one.exe\"'"));
    }

    #[test]
    fn script_refuses_a_file_name_that_could_break_out_of_the_string() {
        let result = installer_script(
            Channel::Production,
            "0.1.0",
            Path::new("out.exe"),
            Path::new("stage"),
            &["a\".dll".to_string()],
            Path::new("icon.ico"),
        );
        assert!(result.is_err());
    }
}
