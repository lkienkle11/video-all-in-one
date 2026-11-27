use super::{path_var, write_tool, TempDir};
use crate::binary_resolver::{bundled_tool_dir, resolve_in_with_bundled};
use vaio_core::domain::tool::Tool;

#[test]
fn bundled_wins_over_a_different_tool_on_path() {
    let tmp = TempDir::new("bundled-wins");
    let bundled = tmp.sub("bundled");
    let configured = tmp.sub("configured");
    let on_path = tmp.sub("on-path");
    let expected = write_tool(&bundled, Tool::Ffmpeg, true);
    write_tool(&configured, Tool::Ffmpeg, true);
    write_tool(&on_path, Tool::Ffmpeg, true);

    let found = resolve_in_with_bundled(
        Tool::Ffmpeg,
        Some(&bundled),
        Some(&configured),
        Some(&path_var(&[&on_path])),
    );

    assert_eq!(found, Ok(expected));
}

#[test]
fn configured_folder_is_used_when_nothing_is_bundled() {
    let tmp = TempDir::new("bundled-none");
    let bundled = tmp.sub("bundled");
    let configured = tmp.sub("configured");
    let on_path = tmp.sub("on-path");
    let expected = write_tool(&configured, Tool::Ffprobe, true);
    write_tool(&on_path, Tool::Ffprobe, true);

    let found = resolve_in_with_bundled(
        Tool::Ffprobe,
        Some(&bundled),
        Some(&configured),
        Some(&path_var(&[&on_path])),
    );

    assert_eq!(found, Ok(expected));
}

#[test]
fn path_is_the_last_resort() {
    let tmp = TempDir::new("bundled-last");
    let bundled = tmp.sub("bundled");
    let on_path = tmp.sub("on-path");
    let expected = write_tool(&on_path, Tool::Ffmpeg, true);

    let found = resolve_in_with_bundled(
        Tool::Ffmpeg,
        Some(&bundled),
        None,
        Some(&path_var(&[&on_path])),
    );

    assert_eq!(found, Ok(expected));
}

#[test]
fn bundled_dir_is_none_when_the_folder_is_absent() {
    let tmp = TempDir::new("bundled-absent");
    let exe = tmp.path().join("App.app/Contents/MacOS/video-all-in-one");
    assert_eq!(bundled_tool_dir(&exe, "macos"), None);
    assert_eq!(bundled_tool_dir(&exe, "windows"), None);
    assert_eq!(bundled_tool_dir(&exe, "linux"), None);
}

#[test]
fn bundled_dir_per_os_layout() {
    let tmp = TempDir::new("bundled-layout");
    let root = tmp.path();

    let mac = root.join("mac/App.app/Contents");
    let mac_bin = mac.join("Resources/bin");
    std::fs::create_dir_all(&mac_bin).unwrap();
    assert_eq!(
        bundled_tool_dir(&mac.join("MacOS/video-all-in-one"), "macos"),
        Some(mac_bin)
    );

    let win = root.join("win");
    let win_bin = win.join("bin");
    std::fs::create_dir_all(&win_bin).unwrap();
    assert_eq!(
        bundled_tool_dir(&win.join("video-all-in-one.exe"), "windows"),
        Some(win_bin)
    );

    let linux = root.join("linux/usr");
    let linux_bin = linux.join("lib/video-all-in-one/bin");
    std::fs::create_dir_all(&linux_bin).unwrap();
    assert_eq!(
        bundled_tool_dir(&linux.join("bin/video-all-in-one"), "linux"),
        Some(linux_bin)
    );
}

#[test]
fn the_dev_linux_package_finds_its_own_tool_folder() {
    let tmp = TempDir::new("bundled-dev-linux");
    let prefix = tmp.path().join("usr");
    let dev_bin = prefix.join("lib/video-all-in-one-dev/bin");
    std::fs::create_dir_all(&dev_bin).unwrap();
    assert_eq!(
        bundled_tool_dir(&prefix.join("bin/video-all-in-one-dev"), "linux"),
        Some(dev_bin)
    );
}
