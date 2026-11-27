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
fn script_inserts_get_parent_at_global_scope_before_its_first_use() {
    let script = script();
    let include = at(&script, "!include FileFunc.nsh\n");
    let insert = at(&script, "!insertmacro GetParent\n");
    let section = at(&script, "Section \"Install\"");
    let first_use = at(&script, "${GetParent}");
    assert!(include < insert && insert < section && section < first_use);
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
    assert!(script.contains(
        "CreateShortcut \"$SMPROGRAMS\\Video All-in-One.lnk\" \"$INSTDIR\\video-all-in-one.exe\""
    ));
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

fn upgrade_script() -> String {
    installer_script(
        Channel::Production,
        "1.2.3",
        Path::new("installer.exe"),
        Path::new("stage"),
        &[
            "video-all-in-one.exe".into(),
            "libmpv-2.dll".into(),
            "third-party\\SOURCES.txt".into(),
        ],
        Path::new("icon.ico"),
    )
    .unwrap()
}

fn at(script: &str, needle: &str) -> usize {
    script
        .find(needle)
        .unwrap_or_else(|| panic!("`{needle}` is missing from:\n{script}"))
}

#[test]
fn script_waits_for_the_executable_to_be_writable() {
    let script = upgrade_script();
    let exe = "$INSTDIR\\video-all-in-one.exe";
    let exists = at(&script, &format!("IfFileExists \"{exe}\""));
    let open = at(&script, &format!("FileOpen $R1 \"{exe}\" a"));
    let sleep = at(&script, "Sleep 500");
    let limit = at(&script, "IntCmp $R0 120");
    let message = at(&script, "SetErrorLevel 2");
    let first_file = at(&script, "File \"stage\\");
    assert!(exists < open && open < limit && limit < sleep && sleep < message);
    assert!(
        message < first_file,
        "the wait comes before any file is copied"
    );
    assert!(script.contains("Abort \""), "{script}");
}

#[test]
fn script_deletes_exactly_the_files_listed_in_the_previous_manifest() {
    let script = upgrade_script();
    let manifest = "$INSTDIR\\install-manifest.txt";
    let exists = at(&script, &format!("IfFileExists \"{manifest}\""));
    let read = at(&script, &format!("FileOpen $R2 \"{manifest}\" r"));
    // The backup reads its own manifest first; inspect the old-file deletion loop.
    let line = read + at(&script[read..], "FileRead $R2 $R3");
    let delete = at(&script, "Delete \"$INSTDIR\\$R3\"");
    let first_file = at(&script, "File \"stage\\");
    assert!(exists < read && read < line && line < delete && delete < first_file);
    assert!(at(&script, "waitdone:") < exists);
    assert!(
        at(
            &script,
            "CopyFiles /SILENT /FILESONLY \"$INSTDIR\\$R3\" \"$R8\""
        ) < delete
    );
    assert_eq!(
        script.matches("Delete \"$INSTDIR\\$R3\"").count(),
        1,
        "one delete of a manifest entry"
    );
}

#[test]
fn script_never_deletes_a_folder_tree_during_an_upgrade() {
    let script = upgrade_script();
    assert!(!script.contains("RMDir /r"));
    for line in script.lines().filter(|l| l.contains("RMDir")) {
        assert!(!line.contains("$R3"), "{line}");
        assert!(!line.contains("/r"), "{line}");
    }
}

#[test]
fn manifest_entries_with_dotdot_or_absolute_paths_are_not_deleted() {
    let script = upgrade_script();
    let delete = at(&script, "Delete \"$INSTDIR\\$R3\"");
    for guard in [
        "StrCmp $R3 \"\" oldloop",
        "StrCmp $R6 \"\\\" oldloop",
        "StrCmp $R6 \"/\" oldloop",
        "StrCmp $R6 \":\" oldloop",
        "StrCmp $R7 \".\" oldloop",
    ] {
        assert!(
            at(&script, guard) < delete,
            "{guard} must come before the delete"
        );
    }
    let before = &script[..delete];
    assert!(before.trim_end().ends_with("scandone:"), "{before}");
    assert!(!script[..at(&script, "oldloop:")].contains("Delete \"$INSTDIR\\$R3\""));
}

#[test]
fn script_writes_a_manifest_of_every_installed_file() {
    let script = upgrade_script();
    let open = at(&script, "FileOpen $R2 \"$INSTDIR\\install-manifest.txt\" w");
    let install_end = at(&script, "Section \"Uninstall\"");
    assert!(open > at(&script, "File \"stage\\bin\\ffprobe.exe\"") && open < install_end);
    for name in [
        "video-all-in-one.exe",
        "libmpv-2.dll",
        "third-party\\SOURCES.txt",
        "bin\\libmpv-2.dll",
        "bin\\ffmpeg.exe",
        "bin\\ffprobe.exe",
    ] {
        let write = at(&script, &format!("FileWrite $R2 \"{name}$\\r$\\n\""));
        assert!(write > open && write < install_end, "{name}");
    }
    assert!(!script.contains("FileWrite $R2 \"uninstall.exe"));
    assert!(!script.contains("FileWrite $R2 \"install-manifest.txt"));
}

#[test]
fn uninstaller_deletes_the_manifest() {
    let script = upgrade_script();
    let uninstall = at(&script, "Section \"Uninstall\"");
    let delete = at(&script, "Delete \"$INSTDIR\\install-manifest.txt\"");
    assert!(delete > uninstall);
}

#[test]
fn script_copies_the_old_manifest_files_to_previous_before_writing() {
    let script = upgrade_script();
    let backup = at(
        &script,
        "CopyFiles /SILENT /FILESONLY \"$INSTDIR\\$R3\" \"$R8\"",
    );
    assert!(at(&script, "waitdone:") < backup);
    assert!(backup < at(&script, "Delete \"$INSTDIR\\$R3\""));
    assert!(backup < at(&script, "File \"stage\\"));
    assert!(script.contains(
        "CopyFiles /SILENT /FILESONLY \"$INSTDIR\\install-manifest.txt\" \"$INSTDIR\\.previous\""
    ));
    assert!(
        script.contains("${GetParent} \"$INSTDIR\\.previous\\$R3\" $R8\n  CreateDirectory \"$R8\"")
    );
    for prefix in ["backup", "old", "restore", "cleanup"] {
        for guard in [
            "StrCmp $R3 \"\"",
            "StrCmp $R6 \"\\\"",
            "StrCmp $R6 \"/\"",
            "StrCmp $R6 \":\"",
            "StrCmp $R7 \".\"",
        ] {
            assert!(
                script.contains(&format!("{guard} {prefix}loop")),
                "{prefix}: {guard}"
            );
        }
    }
}

#[test]
fn script_restores_the_previous_files_when_a_step_fails() {
    let script = upgrade_script();
    let failure = &script[at(&script, "Function .onInstFailed")..];
    assert!(failure.contains("StrCmp $UpdateWriting 1 0 reporterror"));
    let remove = at(failure, "Delete \"$INSTDIR\\third-party\\SOURCES.txt\"");
    let read = at(
        failure,
        "FileOpen $R2 \"$INSTDIR\\.previous\\install-manifest.txt\" r",
    );
    let restore = at(
        failure,
        "CopyFiles /SILENT /FILESONLY \"$INSTDIR\\.previous\\$R3\" \"$R8\"",
    );
    assert!(remove < read && read < restore);
    assert!(failure.contains(
        "CopyFiles /SILENT /FILESONLY \"$INSTDIR\\.previous\\install-manifest.txt\" \"$INSTDIR\""
    ));
    assert!(!failure.contains("Delete \"$INSTDIR\\.previous"));
    let install = &script[..at(&script, "Section \"Uninstall\"")];
    for (index, line) in install
        .lines()
        .enumerate()
        .filter(|(_, l)| l.trim_start().starts_with("File \""))
    {
        let lines: Vec<_> = install.lines().collect();
        assert_eq!(lines[index - 1].trim(), "ClearErrors", "{line}");
        assert_eq!(lines[index + 1].trim(), "IfErrors 0 +2", "{line}");
        assert_eq!(
            lines[index + 2].trim(),
            "Abort \"$UpdateFailure\"",
            "{line}"
        );
    }
    assert!(install.contains("CopyFiles /SILENT /FILESONLY \"$INSTDIR\\$R3\" \"$R8\"\n  IfErrors 0 +2\n  Abort \"$UpdateFailure\""));
    assert!(at(install, "backupdone:") < at(install, "File \"stage\\"));
}

#[test]
fn script_writes_update_error_txt_in_the_data_folder_on_failure() {
    let script = upgrade_script();
    let failure = &script[at(&script, "Function .onInstFailed")..];
    assert!(failure.contains("CreateDirectory \"$APPDATA\\app.videoallinone\""));
    assert!(failure.contains("FileOpen $R2 \"$APPDATA\\app.videoallinone\\update-error.txt\" w"));
    assert!(failure.contains("The update could not be completed.$\\r$\\n$\\r$\\nWhat failed: $UpdateFailure$\\r$\\nThe previous app version is at: $INSTDIR\\.previous$\\r$\\n"));
    assert!(failure.contains("FileClose $R2"));
}

#[test]
fn script_deletes_previous_file_by_file_on_success() {
    let script = upgrade_script();
    let success = script[at(&script, "Function .onInstSuccess")..]
        .split("FunctionEnd")
        .next()
        .unwrap();
    let read = at(
        success,
        "FileOpen $R2 \"$INSTDIR\\.previous\\install-manifest.txt\" r",
    );
    let delete = at(success, "Delete \"$INSTDIR\\.previous\\$R3\"");
    let manifest = at(
        success,
        "Delete \"$INSTDIR\\.previous\\install-manifest.txt\"",
    );
    let directory = at(success, "RMDir \"$INSTDIR\\.previous\"");
    assert!(read < delete && delete < manifest && manifest < directory);
    assert!(directory < at(success, "IfSilent"));
    assert!(success.contains("Exec '\"$INSTDIR\\video-all-in-one.exe\"'"));
}

#[test]
fn script_never_removes_a_folder_tree_for_previous() {
    let script = upgrade_script();
    assert!(
        script.contains("RMDir \"$INSTDIR\\.previous\""),
        "previous cleanup is missing"
    );
    assert!(!script.to_ascii_lowercase().contains("rmdir /r"));
}

#[test]
fn fresh_install_skips_the_backup() {
    let script = upgrade_script();
    let guard = at(
        &script,
        "IfFileExists \"$INSTDIR\\install-manifest.txt\" 0 backupdone",
    );
    assert!(guard < at(&script, "CreateDirectory \"$INSTDIR\\.previous\""));
    assert!(at(&script, "StrCpy $UpdateWriting 1") < at(&script, "backupdone:"));
    let success = &script[at(&script, "Function .onInstSuccess")..];
    assert!(success.contains("StrCmp $UpdateBackup 1 0 cleanupdone"));
    let failure = &script[at(&script, "Function .onInstFailed")..];
    assert!(failure.contains("StrCmp $UpdateBackup 1 0 faileddone"));
}

#[test]
fn data_folder_in_the_script_matches_the_apps_data_folder_for_both_channels() {
    for channel in [Channel::Production, Channel::Development] {
        let script = installer_script(
            channel,
            "1.2.3",
            Path::new("out.exe"),
            Path::new("stage"),
            &[format!("{}.exe", channel.binary_name())],
            Path::new("icon.ico"),
        )
        .unwrap();
        let data = vaio_platform::paths::data_dir(channel, Path::new("$APPDATA"));
        assert_eq!(data.file_name().unwrap(), channel.identifier());
        assert!(script.contains(&format!(
            "FileOpen $R2 \"$APPDATA\\{}\\update-error.txt\" w",
            data.file_name().unwrap().to_str().unwrap()
        )));
    }
}
