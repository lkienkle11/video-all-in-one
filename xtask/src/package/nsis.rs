//! The Windows package: an NSIS installer script generated from a template and built with
//! `makensis`. The installer is per user (no administrator rights) and installs silently
//! with `/S`, which is how the app's updater runs it.

use super::{package_file_name, production_icon, run_tool, APP_NAME, BINARY_NAME, IDENTIFIER};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

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
    version: &str,
    output: &Path,
    source_dir: &Path,
    files: &[String],
    icon: &Path,
) -> Result<String> {
    for file in files {
        check_file_name(file)?;
    }
    let exe = format!("$INSTDIR\\{BINARY_NAME}.exe");
    let shortcut = format!("$SMPROGRAMS\\{APP_NAME}.lnk");
    let key = format!("\"Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{IDENTIFIER}\"");
    let mut lines: Vec<String> = vec![
        "Unicode true".into(),
        "RequestExecutionLevel user".into(),
        format!("Name \"{APP_NAME}\""),
        format!("OutFile {}", quote(&output.display().to_string())?),
        format!("InstallDir \"$LOCALAPPDATA\\Programs\\{APP_NAME}\""),
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
        let path = format!("{}\\{file}", source_dir.display());
        lines.push(format!("  File {}", quote(&path)?));
    }
    lines.extend([
        "  WriteUninstaller \"$INSTDIR\\uninstall.exe\"".to_string(),
        format!("  CreateShortcut \"{shortcut}\" \"{exe}\""),
        format!("  WriteRegStr HKCU {key} \"DisplayName\" \"{APP_NAME}\""),
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
    version: &str,
    key: &str,
    libs: Option<&Path>,
) -> Result<()> {
    let Some(libs) = libs else {
        bail!("--libs is required on Windows: the folder with libmpv-2.dll and the DLLs it needs");
    };
    let stage = out.join("stage-windows");
    if stage.exists() {
        fs::remove_dir_all(&stage).with_context(|| format!("clearing {}", stage.display()))?;
    }
    fs::create_dir_all(&stage)?;
    let exe_name = format!("{BINARY_NAME}.exe");
    fs::copy(binary, stage.join(&exe_name))
        .with_context(|| format!("copying {}", binary.display()))?;
    let mut files = vec![exe_name];
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
        }
    }
    if !files
        .iter()
        .any(|name| name.eq_ignore_ascii_case("libmpv-2.dll"))
    {
        bail!("{} has no libmpv-2.dll", libs.display());
    }
    let installer = out.join(package_file_name(version, key, "exe"));
    if installer.exists() {
        fs::remove_file(&installer)?;
    }
    // The icon sits next to the script, outside the folder of installed files.
    let icon = out.join("icon.ico");
    let icon_source = production_icon(root, "icon.ico")?;
    fs::copy(&icon_source, &icon).with_context(|| format!("copying {}", icon_source.display()))?;
    let script = installer_script(version, &installer, &stage, &files, &icon)?;
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

    fn script() -> String {
        installer_script(
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
            "0.1.0",
            Path::new("out.exe"),
            Path::new("stage"),
            &["a\".dll".to_string()],
            Path::new("icon.ico"),
        );
        assert!(result.is_err());
    }
}
