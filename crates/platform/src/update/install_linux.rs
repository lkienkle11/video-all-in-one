use std::{path::Path, process::Command};

/// `pkexec <this executable> --apply-deb <deb> <sha256>`: the root side re-hashes
/// its own private copy (see `steps::deb`), so no path is trusted after this point.
pub(super) fn installer_command(executable: &Path, deb: &Path, sha256: &[u8; 32]) -> Command {
    let mut command = Command::new("pkexec");
    command
        .arg(executable)
        .arg("--apply-deb")
        .arg(deb)
        .arg(super::steps::hex(sha256));
    command
}

#[cfg(target_os = "linux")]
pub(super) fn install(
    staged: &vaio_core::update::StagedFile,
) -> Result<super::InstallOutcome, vaio_core::update::UpdateError> {
    let path = staged.path.canonicalize().map_err(super::install_error)?;
    let executable = std::env::current_exe()
        .map_err(super::install_error)?
        .canonicalize()
        .map_err(super::install_error)?;
    match installer_command(&executable, &path, &staged.sha256).status() {
        Ok(status) if status.success() => {
            let _ = std::fs::remove_file(&path); // used successfully
            Command::new(executable)
                .spawn()
                .map_err(super::install_error)?;
            Ok(super::InstallOutcome::RestartScheduled)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            super::fallback::reveal(&path)
        }
        Err(error) => Err(super::install_error(error)),
        Ok(status) => Err(super::install_error(format!(
            "package installer exited with {status}"
        ))),
    }
}
