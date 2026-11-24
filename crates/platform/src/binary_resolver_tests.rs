use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use vaio_core::domain::tool::Tool;

use crate::binary_resolver::{
    candidate_names, install_hint, resolve_in, resolve_tool_paths_in, ResolveError,
};

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

/// A temporary folder that is removed when the guard is dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let n = NEXT_DIR.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("vaio-resolver-{label}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        TempDir(path)
    }

    #[allow(dead_code)]
    fn path(&self) -> &Path {
        &self.0
    }

    fn sub(&self, name: &str) -> PathBuf {
        let dir = self.0.join(name);
        std::fs::create_dir_all(&dir).expect("create sub dir");
        dir
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The executable file name of `tool` on the platform running the tests.
fn exe_name(tool: Tool) -> String {
    candidate_names(tool, cfg!(windows)).remove(0)
}

/// Writes an empty file named like `tool` into `dir`; executable when asked.
fn write_tool(dir: &Path, tool: Tool, executable: bool) -> PathBuf {
    let path = dir.join(exe_name(tool));
    std::fs::write(&path, b"").expect("write fake tool");
    set_mode(&path, if executable { 0o755 } else { 0o644 });
    path
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("set permissions");
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}

fn path_var(dirs: &[&Path]) -> std::ffi::OsString {
    std::env::join_paths(dirs).expect("join path entries")
}

/// The path from `base` to `target` using `..` where needed, so a relative
/// PATH entry can point at a temporary folder.
fn relative_from(base: &Path, target: &Path) -> PathBuf {
    let base: Vec<_> = base.components().collect();
    let target: Vec<_> = target.components().collect();
    let common = base.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let mut out = PathBuf::new();
    for _ in common..base.len() {
        out.push("..");
    }
    for part in &target[common..] {
        out.push(part.as_os_str());
    }
    out
}

#[test]
fn configured_directory_wins_over_path() {
    let tmp = TempDir::new("configured");
    let configured = tmp.sub("configured");
    let on_path = tmp.sub("on-path");
    let expected = write_tool(&configured, Tool::Ffmpeg, true);
    write_tool(&on_path, Tool::Ffmpeg, true);

    let found = resolve_in(
        Tool::Ffmpeg,
        Some(&configured),
        Some(&path_var(&[&on_path])),
    );

    assert_eq!(found, Ok(expected));
}

#[test]
fn path_entries_are_searched_in_order() {
    let tmp = TempDir::new("order");
    let first = tmp.sub("first");
    let second = tmp.sub("second");
    let third = tmp.sub("third");
    let expected = write_tool(&second, Tool::YtDlp, true);
    write_tool(&third, Tool::YtDlp, true);

    let found = resolve_in(
        Tool::YtDlp,
        None,
        Some(&path_var(&[&first, &second, &third])),
    );

    assert_eq!(found, Ok(expected));
}

#[test]
fn missing_tool_gives_a_hint_naming_the_tool() {
    let tmp = TempDir::new("missing");
    let empty = tmp.sub("empty");

    let err = resolve_in(Tool::YtDlp, None, Some(&path_var(&[&empty])))
        .expect_err("yt-dlp is not in the folder");

    let expected_hint = install_hint(Tool::YtDlp, std::env::consts::OS);
    assert_eq!(
        err,
        ResolveError::Missing {
            tool: Tool::YtDlp,
            hint: expected_hint.clone(),
        }
    );
    let text = err.to_string();
    assert!(text.contains("yt-dlp"), "message names the tool: {text}");
    assert!(
        text.contains(&expected_hint),
        "message has the hint: {text}"
    );
}

#[cfg(unix)]
#[test]
fn non_executable_file_is_skipped() {
    let tmp = TempDir::new("non-exec");
    let dir = tmp.sub("dir");
    write_tool(&dir, Tool::Ffmpeg, false);

    let found = resolve_in(Tool::Ffmpeg, None, Some(&path_var(&[&dir])));

    assert!(matches!(
        found,
        Err(ResolveError::Missing {
            tool: Tool::Ffmpeg,
            ..
        })
    ));
}

#[test]
fn directory_named_like_the_tool_is_skipped() {
    let tmp = TempDir::new("dir-named");
    let dir = tmp.sub("dir");
    std::fs::create_dir_all(dir.join(exe_name(Tool::Ffmpeg))).expect("create decoy dir");

    let found = resolve_in(Tool::Ffmpeg, None, Some(&path_var(&[&dir])));

    assert!(matches!(
        found,
        Err(ResolveError::Missing {
            tool: Tool::Ffmpeg,
            ..
        })
    ));
}

#[test]
fn relative_and_empty_path_entries_are_ignored() {
    let tmp = TempDir::new("relative");
    let tools = tmp.sub("tools");
    write_tool(&tools, Tool::Ffmpeg, true);
    let cwd = std::env::current_dir().expect("current dir");
    let relative_entry = relative_from(&cwd, &tools);
    assert!(relative_entry.is_relative(), "test entry must be relative");

    let with_relative = path_var(&[&relative_entry]);
    let found = resolve_in(Tool::Ffmpeg, None, Some(&with_relative));
    assert!(matches!(
        found,
        Err(ResolveError::Missing {
            tool: Tool::Ffmpeg,
            ..
        })
    ));

    let with_empty = path_var(&[Path::new(""), &relative_entry]);
    let found = resolve_in(Tool::Ffmpeg, None, Some(&with_empty));
    assert!(matches!(
        found,
        Err(ResolveError::Missing {
            tool: Tool::Ffmpeg,
            ..
        })
    ));
}

#[test]
fn relative_configured_directory_is_rejected() {
    let found = resolve_in(Tool::Ffmpeg, Some(Path::new("tools")), None);

    assert_eq!(
        found,
        Err(ResolveError::ConfiguredDirNotAbsolute(PathBuf::from(
            "tools"
        )))
    );
}

#[cfg(unix)]
#[test]
fn symlink_to_an_executable_is_accepted() {
    let tmp = TempDir::new("symlink");
    let real = tmp.sub("real");
    let links = tmp.sub("links");
    let target = write_tool(&real, Tool::Ffmpeg, true);
    let link = links.join(exe_name(Tool::Ffmpeg));
    std::os::unix::fs::symlink(&target, &link).expect("create symlink");

    let found = resolve_in(Tool::Ffmpeg, None, Some(&path_var(&[&links])));

    assert_eq!(found, Ok(link));
}

#[cfg(unix)]
#[test]
fn broken_symlink_is_skipped() {
    let tmp = TempDir::new("broken");
    let links = tmp.sub("links");
    let missing_target = tmp.path().join("does-not-exist");
    let link = links.join(exe_name(Tool::Ffmpeg));
    std::os::unix::fs::symlink(&missing_target, &link).expect("create broken symlink");

    let found = resolve_in(Tool::Ffmpeg, None, Some(&path_var(&[&links])));

    assert!(matches!(
        found,
        Err(ResolveError::Missing {
            tool: Tool::Ffmpeg,
            ..
        })
    ));
}

#[test]
fn ffprobe_is_not_found_as_ffmpeg() {
    let tmp = TempDir::new("ffprobe");
    let dir = tmp.sub("only-ffmpeg");
    write_tool(&dir, Tool::Ffmpeg, true);

    let found = resolve_in(Tool::Ffprobe, None, Some(&path_var(&[&dir])));

    assert!(matches!(
        found,
        Err(ResolveError::Missing {
            tool: Tool::Ffprobe,
            ..
        })
    ));
}

#[test]
fn candidate_names_add_exe_on_windows() {
    assert_eq!(candidate_names(Tool::Ffmpeg, true), vec!["ffmpeg.exe"]);
    assert_eq!(candidate_names(Tool::Ffprobe, true), vec!["ffprobe.exe"]);
    assert_eq!(candidate_names(Tool::YtDlp, true), vec!["yt-dlp.exe"]);
    assert_eq!(candidate_names(Tool::Ffmpeg, false), vec!["ffmpeg"]);
    assert_eq!(candidate_names(Tool::Ffprobe, false), vec!["ffprobe"]);
    assert_eq!(candidate_names(Tool::YtDlp, false), vec!["yt-dlp"]);
}

#[test]
fn install_hints_for_each_os_name_the_right_command() {
    assert!(install_hint(Tool::Ffmpeg, "macos").contains("brew install ffmpeg"));
    assert!(install_hint(Tool::Ffprobe, "macos").contains("brew install ffmpeg"));
    assert!(install_hint(Tool::YtDlp, "macos").contains("brew install yt-dlp"));
    assert!(install_hint(Tool::Ffmpeg, "windows").contains("winget install ffmpeg"));
    assert!(install_hint(Tool::Ffprobe, "windows").contains("winget install ffmpeg"));
    assert!(install_hint(Tool::YtDlp, "windows").contains("winget install yt-dlp"));
    assert!(install_hint(Tool::Ffmpeg, "linux").contains("sudo apt install ffmpeg"));
    assert!(install_hint(Tool::Ffprobe, "linux").contains("sudo apt install ffmpeg"));
    assert!(install_hint(Tool::YtDlp, "linux").contains("pipx install yt-dlp"));
    assert!(install_hint(Tool::YtDlp, "freebsd").contains("pipx install yt-dlp"));
    assert!(install_hint(Tool::Ffmpeg, "macos").starts_with("ffmpeg"));
}

#[test]
fn resolve_tool_paths_needs_ffmpeg_and_ffprobe_but_not_yt_dlp() {
    let tmp = TempDir::new("tool-paths-optional");
    let dir = tmp.sub("tools");
    let ffmpeg = write_tool(&dir, Tool::Ffmpeg, true);
    let ffprobe = write_tool(&dir, Tool::Ffprobe, true);
    let pv = path_var(&[&dir]);

    let without_yt = resolve_tool_paths_in(None, Some(&pv)).expect("ffmpeg and ffprobe exist");
    assert_eq!(without_yt.ffmpeg, ffmpeg);
    assert_eq!(without_yt.ffprobe, ffprobe);
    assert_eq!(without_yt.yt_dlp, None);

    let yt = write_tool(&dir, Tool::YtDlp, true);
    let with_yt = resolve_tool_paths_in(None, Some(&pv)).expect("all three exist");
    assert_eq!(with_yt.yt_dlp, Some(yt));
}

#[test]
fn resolve_tool_paths_reports_ffmpeg_first() {
    let tmp = TempDir::new("ffmpeg-first");
    let empty = tmp.sub("empty");

    let err =
        resolve_tool_paths_in(None, Some(&path_var(&[&empty]))).expect_err("nothing is installed");

    assert!(matches!(
        err,
        ResolveError::Missing {
            tool: Tool::Ffmpeg,
            ..
        }
    ));
}

#[test]
fn none_path_var_with_no_configured_dir_is_missing() {
    let found = resolve_in(Tool::Ffmpeg, None, None::<&OsStr>);

    assert!(matches!(
        found,
        Err(ResolveError::Missing {
            tool: Tool::Ffmpeg,
            ..
        })
    ));
}
