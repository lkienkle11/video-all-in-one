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
