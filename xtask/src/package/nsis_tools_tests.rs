use super::*;
use crate::package::Channel;
use std::path::Path;

fn at(script: &str, needle: &str) -> usize {
    script
        .find(needle)
        .unwrap_or_else(|| panic!("`{needle}` is missing from:\n{script}"))
}

#[test]
fn tool_dlls_go_to_bin_only_and_libmpv_is_not_duplicated_there() {
    let script = installer_script(
        Channel::Production,
        "1.2.3",
        Path::new("installer.exe"),
        Path::new("stage"),
        &[
            "video-all-in-one.exe".into(),
            "libmpv-2.dll".into(),
            "bin\\avcodec-62.dll".into(),
            "third-party\\SOURCES.txt".into(),
        ],
        Path::new("icon.ico"),
    )
    .unwrap();
    let install = &script[..at(&script, "Section \"Uninstall\"")];
    assert!(install.contains("File \"stage\\bin\\avcodec-62.dll\""));
    assert!(install.contains("FileWrite $R2 \"bin\\avcodec-62.dll$\\r$\\n\""));
    assert!(!install.contains("FileWrite $R2 \"bin\\libmpv-2.dll"));
    assert!(script.contains("Delete \"$INSTDIR\\bin\\avcodec-62.dll\""));
    assert!(!script.contains("Delete \"$INSTDIR\\bin\\libmpv-2.dll\""));
}

/// Splits a script line into words, keeping a quoted text as one word.
fn words(line: &str) -> Vec<String> {
    let (mut out, mut current, mut quote) = (Vec::new(), String::new(), None::<char>);
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => current.push(c),
            None if c == '"' || c == '\'' || c == '`' => quote = Some(c),
            None if c.is_whitespace() => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            None => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// makensis refuses a jump to a label that is not in the same function or section, and the only
/// other check of the script is a real Windows build, so every jump target is checked here.
#[test]
fn every_jump_target_exists_in_its_own_function_or_section() {
    let script = installer_script(
        Channel::Production,
        "1.2.3",
        Path::new("installer.exe"),
        Path::new("stage"),
        &[
            "video-all-in-one.exe".into(),
            "libmpv-2.dll".into(),
            "bin\\avcodec-62.dll".into(),
            "third-party\\SOURCES.txt".into(),
        ],
        Path::new("icon.ico"),
    )
    .unwrap();
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    for line in script.lines() {
        let head = line.trim_start();
        if head.starts_with("Function ") || head.starts_with("Section") {
            blocks.push(Vec::new());
        }
        if let Some(block) = blocks.last_mut() {
            block.push(line);
        }
    }
    assert!(blocks.len() >= 3);
    for block in &blocks {
        let labels: Vec<String> = block
            .iter()
            .filter_map(|l| l.trim().strip_suffix(':').map(str::to_owned))
            .collect();
        for line in block {
            let w = words(line);
            let targets: Vec<&String> = match w.first().map(String::as_str) {
                Some("Goto") => w.iter().skip(1).collect(),
                Some("IfErrors" | "IfSilent") => w.iter().skip(1).collect(),
                Some("StrCmp" | "StrCmpS" | "IntCmp") => w.iter().skip(3).collect(),
                Some("IfFileExists") => w.iter().skip(2).collect(),
                _ => Vec::new(),
            };
            for target in targets {
                let relative = target.starts_with('+') || target.starts_with('-');
                if relative || target == "0" || target.starts_with('$') {
                    continue;
                }
                assert!(
                    labels.contains(target),
                    "`{line}` jumps to `{target}`, which is not a label of {}",
                    block[0]
                );
            }
        }
    }
}
