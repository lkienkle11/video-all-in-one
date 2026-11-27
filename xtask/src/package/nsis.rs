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

/// The list of installed files, one path per line relative to `$INSTDIR`.
const MANIFEST: &str = "install-manifest.txt";

/// Waits up to 60 seconds (120 tries, 500 ms apart) until the installed executable can be
/// opened for writing, so the quitting app has released it. Fails with exit code 2 and a
/// message in the log; there is no dialog, so a silent install never blocks.
fn wait_for_the_executable(exe: &str) -> Vec<String> {
    [
        format!("  IfFileExists \"{exe}\" 0 waitdone"),
        "  StrCpy $R0 0".into(),
        "  waitloop:".into(),
        "  ClearErrors".into(),
        format!("  FileOpen $R1 \"{exe}\" a"),
        "  IfErrors 0 waitopened".into(),
        "  IntOp $R0 $R0 + 1".into(),
        "  IntCmp $R0 120 waitfail 0 waitfail".into(),
        "  Sleep 500".into(),
        "  Goto waitloop".into(),
        "  waitopened:".into(),
        "  FileClose $R1".into(),
        "  Goto waitdone".into(),
        "  waitfail:".into(),
        "  SetErrorLevel 2".into(),
        "  Abort \"The app is still running and its files are in use. Close it and run the installer again.\"".into(),
        "  waitdone:".into(),
    ]
    .into()
}

/// Every manifest operation shares the same path guards; each loop owns its labels.
/// Callers supply the `done` label, so their final steps run only after reading the list.
fn manifest_loop(prefix: &str, manifest: &str, operation: &str) -> String {
    let labels = if prefix == "old" { "" } else { prefix };
    format!(
        r#"  ClearErrors
  FileOpen $R2 "{manifest}" r
  IfErrors {prefix}done
  {prefix}loop:
  ClearErrors
  FileRead $R2 $R3
  IfErrors {prefix}close
  {labels}trimloop:
  StrCpy $R4 $R3 1 -1
  StrCmp $R4 "$\n" {labels}trimcut
  StrCmp $R4 "$\r" {labels}trimcut
  Goto {labels}trimmed
  {labels}trimcut:
  StrCpy $R3 $R3 -1
  Goto {labels}trimloop
  {labels}trimmed:
  StrCmp $R3 "" {prefix}loop
  StrCpy $R6 $R3 1
  StrCmp $R6 "\" {prefix}loop
  StrCmp $R6 "/" {prefix}loop
  StrCpy $R5 0
  {labels}scanloop:
  StrCpy $R6 $R3 1 $R5
  StrCmp $R6 "" {labels}scandone
  StrCmp $R6 ":" {prefix}loop
  StrCmp $R6 "." 0 {labels}scannext
  IntOp $R7 $R5 + 1
  StrCpy $R7 $R3 1 $R7
  StrCmp $R7 "." {prefix}loop
  {labels}scannext:
  IntOp $R5 $R5 + 1
  Goto {labels}scanloop
  {labels}scandone:
{operation}
  Goto {prefix}loop
  {prefix}close:
  FileClose $R2"#
    )
}

/// Record a precise failure before any operation can abort, including an NSIS `File`.
fn checked(operation: &str, what: &str) -> String {
    format!("  StrCpy $UpdateFailure \"{what}\"\n  ClearErrors\n{operation}\n  IfErrors 0 +2\n  Abort \"$UpdateFailure\"")
}

/// Writes the manifest of the files this install put in `$INSTDIR`, not the manifest itself
/// and not `uninstall.exe`.
fn write_the_manifest(files: &[String]) -> Vec<String> {
    let mut lines = vec![format!("  FileOpen $R2 \"$INSTDIR\\{MANIFEST}\" w")];
    let dlls = files
        .iter()
        .filter(|file| file.to_ascii_lowercase().ends_with(".dll"));
    let entries = files
        .iter()
        .cloned()
        .chain(dlls.map(|file| format!("bin\\{file}")))
        .chain([
            "bin\\ffmpeg.exe".to_string(),
            "bin\\ffprobe.exe".to_string(),
        ]);
    for entry in entries {
        lines.push(format!("  FileWrite $R2 \"{entry}$\\r$\\n\""));
    }
    lines.push("  FileClose $R2".into());
    lines
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
        "!include FileFunc.nsh\n!insertmacro GetParent\nVar UpdateBackup\nVar UpdateWriting\nVar UpdateFailure\nVar UpdateRestoreFailed\nVar UpdateRestoreNote".into(),
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
    ];
    lines.extend(wait_for_the_executable(&exe));
    let previous = "$INSTDIR\\.previous";
    let old_manifest = format!("$INSTDIR\\{MANIFEST}");
    let backup_manifest = format!("{previous}\\{MANIFEST}");
    lines.push(format!("  IfFileExists \"{old_manifest}\" 0 backupdone\n  IfFileExists \"{previous}\\*.*\" 0 +2\n  Abort \"A previous update backup already exists at {previous}.\""));
    lines.push("  StrCpy $UpdateBackup 1".into());
    lines.push(checked(&format!("  CreateDirectory \"{previous}\"\n  CopyFiles /SILENT /FILESONLY \"{old_manifest}\" \"{previous}\""), "Backing up the install manifest"));
    let backup = checked("  IfFileExists \"$INSTDIR\\$R3\" 0 backupskip\n  ${GetParent} \"$INSTDIR\\.previous\\$R3\" $R8\n  CreateDirectory \"$R8\"\n  CopyFiles /SILENT /FILESONLY \"$INSTDIR\\$R3\" \"$R8\"\n  backupskip:", "Backing up $INSTDIR\\$R3");
    lines.push(manifest_loop("backup", &backup_manifest, &backup));
    lines.push("  StrCpy $UpdateWriting 1\n  backupdone:".into());
    // An unreadable backup list must never permit deletion of the installed files.
    lines.push("  StrCmp $UpdateBackup 1 0 olddone\n  StrCmp $UpdateWriting 1 +2\n  Abort \"Could not read the previous install manifest.\"".into());
    lines.push(manifest_loop(
        "old",
        &old_manifest,
        "  Delete \"$INSTDIR\\$R3\"",
    ));
    lines.push("  RMDir \"$INSTDIR\\bin\"\n  RMDir \"$INSTDIR\\third-party\"\n  olddone:\n  SetOverwrite try".into());
    lines.push("  SetOutPath \"$INSTDIR\"".into());
    for file in files {
        let destination = if file.starts_with("third-party\\") {
            "$INSTDIR\\third-party"
        } else {
            "$INSTDIR"
        };
        lines.push(format!("  SetOutPath \"{destination}\""));
        let path = format!("{}\\{file}", source_dir.display());
        lines.push(checked(
            &format!("  File {}", quote(&path)?),
            &format!("Installing {file}"),
        ));
    }
    lines.push("  SetOutPath \"$INSTDIR\\bin\"".into());
    for file in files
        .iter()
        .filter(|file| file.to_ascii_lowercase().ends_with(".dll"))
    {
        lines.push(checked(
            &format!(
                "  File {}",
                quote(&format!("{}\\{file}", source_dir.display()))?
            ),
            &format!("Installing bin\\{file}"),
        ));
    }
    for name in ["ffmpeg.exe", "ffprobe.exe"] {
        lines.push(checked(
            &format!(
                "  File {}",
                quote(&format!("{}\\bin\\{name}", source_dir.display()))?
            ),
            &format!("Installing bin\\{name}"),
        ));
    }
    lines.push("  SetOutPath \"$INSTDIR\"".into());
    lines.extend(write_the_manifest(files));
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
    let payload_start = lines.len();
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
    let payload_deletes = lines[payload_start..].to_vec();
    lines.push("  RMDir \"$INSTDIR\\bin\"".into());
    lines.push("  RMDir \"$INSTDIR\\third-party\"".into());
    lines.extend([
        format!("  Delete \"$INSTDIR\\{MANIFEST}\""),
        "  Delete \"$INSTDIR\\uninstall.exe\"".to_string(),
        format!("  Delete \"{shortcut}\""),
        format!("  DeleteRegKey HKCU {key}"),
        "  RMDir \"$INSTDIR\"".to_string(),
        "SectionEnd".to_string(),
        String::new(),
        "Function .onInstSuccess".to_string(),
    ]);
    lines.push("  StrCmp $UpdateBackup 1 0 cleanupdone".into());
    let cleanup_body = "  Delete \"$INSTDIR\\.previous\\$R3\"\n  ${GetParent} \"$INSTDIR\\.previous\\$R3\" $R8\n  cleanupdirs:\n  StrCmp $R8 \"$INSTDIR\\.previous\" cleanupnext\n  RMDir \"$R8\"\n  ${GetParent} \"$R8\" $R8\n  Goto cleanupdirs\n  cleanupnext:";
    lines.push(manifest_loop("cleanup", &backup_manifest, cleanup_body));
    lines.push(format!(
        "  Delete \"{backup_manifest}\"\n  RMDir \"{previous}\"\n  cleanupdone:"
    ));
    lines.extend([
        "  IfSilent 0 done".to_string(),
        format!("  Exec '\"{exe}\"'"),
        "  done:".to_string(),
        "FunctionEnd".to_string(),
    ]);
    lines.push("Function .onInstFailed\n  StrCmp $UpdateBackup 1 0 faileddone\n  FileClose $R2\n  StrCpy $UpdateRestoreFailed 0\n  StrCpy $UpdateRestoreNote \"The previous app version was restored.\"\n  StrCmp $UpdateWriting 1 0 failedrestore".into());
    // Remove only the known new payload before restoring, including newly added files.
    lines.extend(payload_deletes);
    lines.push(manifest_loop("restore", &backup_manifest, "  ${GetParent} \"$INSTDIR\\$R3\" $R8\n  CreateDirectory \"$R8\"\n  ClearErrors\n  CopyFiles /SILENT /FILESONLY \"$INSTDIR\\.previous\\$R3\" \"$R8\"\n  IfErrors 0 +2\n  StrCpy $UpdateFailure \"$UpdateFailure; restoring $R3 also failed\"\n  StrCpy $UpdateRestoreFailed 1"));
    lines.push(format!("  restoredone:\n  ClearErrors\n  CopyFiles /SILENT /FILESONLY \"{backup_manifest}\" \"$INSTDIR\"\n  IfErrors 0 +3\n  StrCpy $UpdateFailure \"$UpdateFailure; restoring the install manifest also failed\"\n  StrCpy $UpdateRestoreFailed 1\n  StrCmp $UpdateRestoreFailed 1 reporterror"));
    // A restore with no error leaves the backup redundant, and a leftover backup would block every retry.
    lines.push(manifest_loop("cleanup", &backup_manifest, cleanup_body));
    lines.push(format!("  Delete \"{backup_manifest}\"\n  RMDir \"{previous}\"\n  Goto reporterror\n  failedrestore:\n  StrCpy $UpdateRestoreFailed 1\n  reporterror:\n  StrCmp $UpdateRestoreFailed 1 0 writeerror\n  StrCpy $UpdateRestoreNote \"The previous app version is at: {previous}\"\n  writeerror:\n  CreateDirectory \"$APPDATA\\{identifier}\"\n  FileOpen $R2 \"$APPDATA\\{identifier}\\update-error.txt\" w\n  IfErrors faileddone\n  FileWrite $R2 \"The update could not be completed.$\\r$\\n$\\r$\\nWhat failed: $UpdateFailure$\\r$\\n$UpdateRestoreNote$\\r$\\n\"\n  FileClose $R2\n  faileddone:\n  SetErrorLevel 2\nFunctionEnd"));
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
#[path = "nsis_tests.rs"]
mod tests;
