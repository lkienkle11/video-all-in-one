//! Finds the FFmpeg, FFprobe and yt-dlp executables in the user's configured
//! folder first and then on the `PATH`.

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};

use vaio_core::domain::tool::Tool;

/// Why a tool could not be resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolveError {
    /// The tool was not found; `hint` names the tool and tells the user how to install it.
    Missing { tool: Tool, hint: String },
    /// The configured folder is not an absolute path.
    ConfiguredDirNotAbsolute(PathBuf),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::Missing { hint, .. } => write!(f, "{hint}"),
            ResolveError::ConfiguredDirNotAbsolute(path) => write!(
                f,
                "the configured tool folder must be an absolute path: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ResolveError {}

/// File names a tool may have on the given platform.
pub fn candidate_names(tool: Tool, windows: bool) -> Vec<String> {
    if windows {
        vec![format!("{}.exe", tool.file_stem())]
    } else {
        vec![tool.file_stem().to_string()]
    }
}

/// One sentence telling the user how to install `tool` on `os`.
pub fn install_hint(tool: Tool, os: &str) -> String {
    let command = match (os, tool) {
        ("macos", Tool::YtDlp) => "brew install yt-dlp",
        ("macos", _) => "brew install ffmpeg",
        ("windows", Tool::YtDlp) => "winget install yt-dlp",
        ("windows", _) => "winget install ffmpeg",
        (_, Tool::YtDlp) => "pipx install yt-dlp",
        (_, _) => "sudo apt install ffmpeg",
    };
    format!(
        "{} was not found. Install it with: {command}, or choose the folder that contains it in Settings.",
        tool.file_stem()
    )
}

/// The folder that ships FFmpeg and FFprobe next to the app, when it exists.
/// `exe` is the running executable and `os` a `std::env::consts::OS` value.
pub fn bundled_tool_dir(exe: &Path, os: &str) -> Option<PathBuf> {
    let dir = match os {
        // <App>.app/Contents/MacOS/<bin> -> <App>.app/Contents/Resources/bin
        "macos" => exe.parent()?.parent()?.join("Resources").join("bin"),
        "windows" => exe.parent()?.join("bin"),
        // <prefix>/bin/<bin> -> <prefix>/lib/video-all-in-one/bin
        _ => exe
            .parent()?
            .parent()?
            .join("lib")
            .join("video-all-in-one")
            .join("bin"),
    };
    dir.is_dir().then_some(dir)
}

/// Finds `tool` in `configured_dir` first, then in the entries of `path_var`.
pub fn resolve_in(
    tool: Tool,
    configured_dir: Option<&Path>,
    path_var: Option<&OsStr>,
) -> Result<PathBuf, ResolveError> {
    resolve_in_with_bundled(tool, None, configured_dir, path_var)
}

/// Finds `tool` in the bundled folder, then `configured_dir`, then the entries of `path_var`.
pub fn resolve_in_with_bundled(
    tool: Tool,
    bundled: Option<&Path>,
    configured_dir: Option<&Path>,
    path_var: Option<&OsStr>,
) -> Result<PathBuf, ResolveError> {
    let names = candidate_names(tool, cfg!(windows));
    if let Some(dir) = bundled {
        if let Some(found) = find_in_dir(dir, &names) {
            return Ok(found);
        }
    }
    if let Some(dir) = configured_dir {
        if !dir.is_absolute() {
            return Err(ResolveError::ConfiguredDirNotAbsolute(dir.to_path_buf()));
        }
        if let Some(found) = find_in_dir(dir, &names) {
            return Ok(found);
        }
    }
    if let Some(var) = path_var {
        for dir in std::env::split_paths(var) {
            // A relative entry such as "." would let a file in the current folder
            // hijack the tool, so only absolute entries are searched.
            if !dir.is_absolute() {
                continue;
            }
            if let Some(found) = find_in_dir(&dir, &names) {
                return Ok(found);
            }
        }
    }
    Err(ResolveError::Missing {
        tool,
        hint: install_hint(tool, std::env::consts::OS),
    })
}

/// Finds `tool` using the real `PATH` of this process.
pub fn resolve(tool: Tool, configured_dir: Option<&Path>) -> Result<PathBuf, ResolveError> {
    let path = std::env::var_os("PATH");
    resolve_in(tool, configured_dir, path.as_deref())
}

/// The resolved executables the media pipeline needs.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolPaths {
    /// The FFmpeg executable (required).
    pub ffmpeg: PathBuf,
    /// The FFprobe executable (required).
    pub ffprobe: PathBuf,
    /// The yt-dlp executable (optional until downloads are used).
    pub yt_dlp: Option<PathBuf>,
}

/// Resolves all tools using the real `PATH`.
pub fn resolve_tool_paths(configured_dir: Option<&Path>) -> Result<ToolPaths, ResolveError> {
    let path = std::env::var_os("PATH");
    resolve_tool_paths_in(configured_dir, path.as_deref())
}

/// Resolves all tools using an explicit `PATH` value (for tests and callers
/// that already hold it).
pub fn resolve_tool_paths_in(
    configured_dir: Option<&Path>,
    path_var: Option<&OsStr>,
) -> Result<ToolPaths, ResolveError> {
    resolve_tool_paths_in_with_bundled(None, configured_dir, path_var)
}

/// Like [`resolve_tool_paths_in`], searching the bundled folder first.
pub fn resolve_tool_paths_in_with_bundled(
    bundled: Option<&Path>,
    configured_dir: Option<&Path>,
    path_var: Option<&OsStr>,
) -> Result<ToolPaths, ResolveError> {
    let ffmpeg = resolve_in_with_bundled(Tool::Ffmpeg, bundled, configured_dir, path_var)?;
    let ffprobe = resolve_in_with_bundled(Tool::Ffprobe, bundled, configured_dir, path_var)?;
    let yt_dlp = match resolve_in_with_bundled(Tool::YtDlp, bundled, configured_dir, path_var) {
        Ok(path) => Some(path),
        Err(ResolveError::Missing { .. }) => None,
        Err(other) => return Err(other),
    };
    Ok(ToolPaths {
        ffmpeg,
        ffprobe,
        yt_dlp,
    })
}

/// The first candidate file name in `dir` that is an executable regular file.
fn find_in_dir(dir: &Path, names: &[String]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| dir.join(name))
        .find(|candidate| is_executable_file(candidate))
}

/// True when `path` is a regular file (after following symlinks) and is executable.
fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    meta.is_file() && is_executable(&meta)
}

#[cfg(unix)]
fn is_executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_meta: &std::fs::Metadata) -> bool {
    true
}
