use super::install_error;
#[cfg(target_os = "macos")]
use super::steps::bundle_rules;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use vaio_core::update::UpdateError;

pub(super) fn bundle_for_executable(executable: &Path) -> Result<PathBuf, UpdateError> {
    let macos = executable
        .parent()
        .ok_or_else(|| install_error("executable has no parent"))?;
    let contents = macos
        .parent()
        .ok_or_else(|| install_error("missing Contents directory"))?;
    let bundle = contents
        .parent()
        .ok_or_else(|| install_error("missing app bundle"))?;
    if macos.file_name() != Some(std::ffi::OsStr::new("MacOS"))
        || contents.file_name() != Some(std::ffi::OsStr::new("Contents"))
        || bundle.extension() != Some(std::ffi::OsStr::new("app"))
    {
        return Err(install_error("the executable is not inside an app bundle"));
    }
    Ok(bundle.to_path_buf())
}

pub(super) fn extract_command(archive: &Path, destination: &Path) -> Command {
    let mut command = Command::new("/usr/bin/ditto");
    command.args(["-x", "-k"]).arg(archive).arg(destination);
    command
}

pub(super) fn helper_command(
    executable: &Path,
    staged: &Path,
    target: &Path,
    pid: u32,
    data_dir: &Path,
) -> Command {
    let mut command = Command::new(executable);
    command
        .arg("--apply-update")
        .arg(staged)
        .arg(target)
        .arg(pid.to_string())
        .arg(data_dir);
    command
}

#[cfg(target_os = "macos")]
pub(super) fn install(
    staged: &vaio_core::update::StagedFile,
    data_dir: &Path,
) -> Result<super::InstallOutcome, UpdateError> {
    use super::steps::private_copy::{copy_verified, create_private_dir, ScopedDir};
    use std::{
        fs,
        os::unix::process::CommandExt,
        process::Stdio,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let executable = std::env::current_exe()
        .map_err(install_error)?
        .canonicalize()
        .map_err(install_error)?;
    let target = match bundle_for_executable(&executable) {
        Ok(target) => target,
        Err(_) => return super::fallback::reveal(&staged.path),
    };
    let parent = target
        .parent()
        .ok_or_else(|| install_error("missing bundle parent"))?;
    // Same filesystem as the target, private, exclusive and never reused.
    let work_path = parent.join(format!(
        ".vaio-update-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    match create_private_dir(&work_path) {
        Ok(()) => {}
        // No write access beside the app (for example a root-owned install):
        // show the download instead of failing.
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return super::fallback::reveal(&staged.path);
        }
        Err(error) => return Err(install_error(error)),
    }
    // Removed on every error path below; disarmed only once the helper owns it.
    let mut work = ScopedDir::new(work_path);
    // Extract a private, re-hashed copy, never the file another process could swap.
    let copy = copy_verified(staged, work.path())?;
    let status = extract_command(copy.path(), work.path())
        .status()
        .map_err(install_error)?;
    drop(copy);
    if !status.success() {
        return Err(install_error("could not extract update archive"));
    }
    let _ = fs::remove_file(&staged.path); // the archive has been used
    let bundles: Vec<_> = fs::read_dir(work.path())
        .map_err(install_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(install_error)?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.extension() == Some(std::ffi::OsStr::new("app")))
        .collect();
    if bundles.len() != 1 {
        return Err(install_error("archive must contain exactly one app bundle"));
    }
    let new_bundle = &bundles[0];
    validate_bundles(new_bundle, &target, &executable)?;
    helper_command(
        &executable,
        new_bundle,
        &target,
        std::process::id(),
        data_dir,
    )
    .process_group(0)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .map_err(install_error)?;
    work.disarm();
    Ok(super::InstallOutcome::RestartScheduled)
}

#[cfg(target_os = "macos")]
fn validate_bundles(staged: &Path, target: &Path, executable: &Path) -> Result<(), UpdateError> {
    let target = target.canonicalize().map_err(install_error)?;
    let current = bundle_for_executable(executable)?
        .canonicalize()
        .map_err(install_error)?;
    let staged = staged.canonicalize().map_err(install_error)?;
    bundle_rules::check_paths(&staged, &target, &current)?;
    let relative = executable.strip_prefix(&target).map_err(install_error)?;
    let new_executable = staged
        .join(relative)
        .canonicalize()
        .map_err(install_error)?;
    if !new_executable.starts_with(&staged) || !new_executable.is_file() {
        return Err(install_error("invalid staged executable"));
    }
    bundle_rules::check_identifiers(&bundle_identifier(&staged)?, &bundle_identifier(&target)?)
}

#[cfg(target_os = "macos")]
fn bundle_identifier(bundle: &Path) -> Result<Vec<u8>, UpdateError> {
    let output = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :CFBundleIdentifier"])
        .arg(bundle.join("Contents/Info.plist"))
        .output()
        .map_err(install_error)?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(install_error("missing bundle identifier"));
    }
    Ok(output.stdout)
}

#[cfg(target_os = "macos")]
fn relaunch(target: &Path) -> Result<(), UpdateError> {
    let status = Command::new("/usr/bin/open")
        .arg("-n")
        .arg(target)
        .status()
        .map_err(install_error)?;
    if status.success() {
        Ok(())
    } else {
        Err(install_error("could not relaunch the app"))
    }
}

#[cfg(target_os = "macos")]
pub(super) fn apply(request: &super::apply::ApplyRequest) -> Result<(), UpdateError> {
    use super::apply::{settle, swap_bundle, wait_for_exit};
    use super::steps::recovery::{recover, write_error_file};
    use std::time::Duration;
    // Undo a swap an earlier helper never finished before starting a new one.
    recover(&request.data_dir);
    let early = |error: UpdateError| {
        let _ = write_error_file(&request.data_dir, &error.to_string(), &request.target);
        error
    };
    let executable = std::env::current_exe()
        .map_err(install_error)
        .and_then(|path| path.canonicalize().map_err(install_error))
        .map_err(early)?;
    validate_bundles(&request.staged, &request.target, &executable).map_err(early)?;
    wait_for_exit(Duration::from_secs(60), || {
        let output = Command::new("/bin/ps")
            .args(["-p", &request.pid.to_string(), "-o", "pid="])
            .output()
            .map_err(install_error)?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) if output.stdout.is_empty() => Ok(false),
            _ => Err(install_error("could not inspect parent process")),
        }
    })
    .map_err(early)?;
    validate_bundles(&request.staged, &request.target, &executable).map_err(early)?;
    let backup = request
        .staged
        .parent()
        .ok_or_else(|| install_error("missing staging root"))
        .map_err(early)?
        .join("previous.app");
    let swapped = swap_bundle(
        &request.staged,
        &request.target,
        &backup,
        &request.data_dir,
        || relaunch(&request.target),
    );
    settle(swapped, &request.data_dir, &request.target, &backup, || {
        relaunch(&request.target)
    })
}
