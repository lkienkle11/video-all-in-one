use super::install_error;
#[cfg(any(target_os = "macos", test))]
use super::steps::recovery::{
    clear_error_file, clear_marker, write_error_file, write_marker, Marker,
};
use std::{ffi::OsString, path::PathBuf};
#[cfg(any(target_os = "macos", test))]
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
use vaio_core::update::UpdateError;

/// `--apply-update <staged> <target> <pid> <data-dir>`: swap the macOS app bundle.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ApplyRequest {
    pub staged: PathBuf,
    pub target: PathBuf,
    pub pid: u32,
    pub data_dir: PathBuf,
}

/// `--apply-deb <path> <sha256>`: the root-side Linux install step.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct DebRequest {
    pub path: PathBuf,
    pub sha256_hex: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum HelperRequest {
    Update(ApplyRequest),
    Deb(DebRequest),
}

/// Helper mode starts only when argument 0 is exactly a helper flag; the same text
/// anywhere else (a file name, a later argument) never triggers it.
pub(super) fn parse(args: &[OsString]) -> Result<Option<HelperRequest>, UpdateError> {
    match args.first().and_then(|arg| arg.to_str()) {
        Some("--apply-update") => {
            parse_update(args).map(|request| Some(HelperRequest::Update(request)))
        }
        Some("--apply-deb") => parse_deb(args).map(|request| Some(HelperRequest::Deb(request))),
        _ => Ok(None),
    }
}

fn parse_update(args: &[OsString]) -> Result<ApplyRequest, UpdateError> {
    if args.len() != 5 {
        return Err(install_error(
            "expected --apply-update <staged> <target> <pid> <data-dir>",
        ));
    }
    let pid = args[3]
        .to_str()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|pid| *pid > 0 && *pid != std::process::id())
        .ok_or_else(|| install_error("invalid parent PID"))?;
    let staged = PathBuf::from(&args[1]);
    let target = PathBuf::from(&args[2]);
    let data_dir = PathBuf::from(&args[4]);
    if !staged.is_absolute() || !target.is_absolute() || !data_dir.is_absolute() || staged == target
    {
        return Err(install_error("update paths must be distinct and absolute"));
    }
    Ok(ApplyRequest {
        staged,
        target,
        pid,
        data_dir,
    })
}

fn parse_deb(args: &[OsString]) -> Result<DebRequest, UpdateError> {
    if args.len() != 3 {
        return Err(install_error("expected --apply-deb <path> <sha256>"));
    }
    let path = PathBuf::from(&args[1]);
    let sha256_hex = args[2]
        .to_str()
        .filter(|text| text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| install_error("expected a 64 digit SHA-256"))?
        .to_ascii_lowercase();
    if !path.is_absolute() {
        return Err(install_error("the package path must be absolute"));
    }
    Ok(DebRequest { path, sha256_hex })
}

/// Returns an exit code for helper mode, or `None` for normal window startup.
/// Arguments exclude argv[0]. No window is created by this function.
pub fn handle_apply_update(args: impl IntoIterator<Item = OsString>) -> Option<i32> {
    let request = match parse(&args.into_iter().collect::<Vec<_>>()) {
        Ok(None) => return None,
        Ok(Some(request)) => request,
        Err(error) => {
            eprintln!("{error}");
            return Some(1);
        }
    };
    if !crate::channel::Channel::current().updates_enabled() {
        eprintln!("updates are disabled in development builds");
        return Some(1);
    }
    match run(request) {
        Ok(()) => Some(0),
        Err(error) => {
            eprintln!("{error}");
            Some(1)
        }
    }
}

fn run(request: HelperRequest) -> Result<(), UpdateError> {
    match request {
        #[cfg(target_os = "macos")]
        HelperRequest::Update(request) => super::install_macos::apply(&request),
        #[cfg(target_os = "linux")]
        HelperRequest::Deb(request) => super::steps::deb::apply(&request),
        #[allow(unreachable_patterns)]
        other => {
            let _ = other;
            Err(install_error(
                "this update helper is not available on this operating system",
            ))
        }
    }
}

#[cfg(any(target_os = "macos", test))]
pub(super) fn wait_for_exit(
    timeout: Duration,
    mut alive: impl FnMut() -> Result<bool, UpdateError>,
) -> Result<(), UpdateError> {
    let start = Instant::now();
    while alive()? {
        let remaining = timeout
            .checked_sub(start.elapsed())
            .ok_or_else(|| install_error("timed out waiting for the app to exit"))?;
        std::thread::sleep(remaining.min(Duration::from_millis(100)));
    }
    Ok(())
}

/// Moves the old bundle aside and the new one into place. A recovery marker is
/// written before the first rename so an interrupted swap can be undone later.
#[cfg(any(target_os = "macos", test))]
pub(super) fn swap_bundle(
    staged: &Path,
    target: &Path,
    backup: &Path,
    data_dir: &Path,
    relaunch: impl FnOnce() -> Result<(), UpdateError>,
) -> Result<(), UpdateError> {
    for path in [staged, target] {
        if path
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(install_error("refused symlink app bundle"));
        }
    }
    if backup.symlink_metadata().is_ok() {
        return Err(install_error("backup already exists"));
    }
    write_marker(
        data_dir,
        &Marker {
            target: target.to_path_buf(),
            backup: backup.to_path_buf(),
        },
    )?;
    // The marker is removed whenever the old bundle is known to be back in place;
    // it stays when it is not, so the next start can finish the restore.
    if let Err(error) = fs::rename(target, backup) {
        clear_marker(data_dir);
        return Err(install_error(error));
    }
    if let Err(error) = fs::rename(staged, target) {
        fs::rename(backup, target).map_err(|restore| {
            install_error(format!(
                "move failed: {error}; restore failed: {restore}; old app retained at {}",
                backup.display()
            ))
        })?;
        clear_marker(data_dir);
        return Err(install_error(error));
    }
    if let Err(error) = relaunch() {
        fs::rename(target, staged).map_err(|restore| {
            install_error(format!(
                "{error}; could not move new app aside: {restore}; old app retained at {}",
                backup.display()
            ))
        })?;
        fs::rename(backup, target).map_err(|restore| {
            install_error(format!(
                "{error}; restore failed: {restore}; old app retained at {}",
                backup.display()
            ))
        })?;
        clear_marker(data_dir);
        return Err(error);
    }
    clear_marker(data_dir);
    Ok(())
}

/// Finishes a swap. On failure it records `update-error.txt` and, if the old app is
/// back in place, starts it again so the user is never left without an app. On
/// success it removes the error file, the old bundle and the staging folder.
#[cfg(any(target_os = "macos", test))]
pub(super) fn settle(
    result: Result<(), UpdateError>,
    data_dir: &Path,
    target: &Path,
    backup: &Path,
    relaunch_old: impl FnOnce() -> Result<(), UpdateError>,
) -> Result<(), UpdateError> {
    match result {
        Ok(()) => {
            clear_error_file(data_dir);
            // Only ever remove a folder this updater made: `.vaio-update-*`.
            if let Some(root) = backup.parent().filter(|root| {
                root.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".vaio-update-"))
            }) {
                let _ = fs::remove_dir_all(root);
            }
            Ok(())
        }
        Err(error) => {
            // Report with the original error; a failed report never replaces it.
            let _ = write_error_file(data_dir, &error.to_string(), backup);
            if target.symlink_metadata().is_ok() {
                let _ = relaunch_old();
            }
            Err(error)
        }
    }
}
