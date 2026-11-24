use std::{path::Path, process::Command};
pub(super) fn installer_command(path: &Path) -> Command {
    let mut command = Command::new(path);
    // NSIS is the selected contract; Task 12 must confirm it. /S is case-sensitive.
    command.arg("/S");
    command
}

#[cfg(target_os = "windows")]
pub(super) fn install(
    staged: &vaio_core::update::StagedFile,
) -> Result<super::InstallOutcome, vaio_core::update::UpdateError> {
    use super::steps::private_copy::copy_verified;
    use std::{os::windows::process::CommandExt, process::Stdio};
    let parent = staged
        .path
        .parent()
        .ok_or_else(|| super::install_error("staged file has no folder"))?;
    // Run the private, re-hashed copy; the original in the work folder is never executed.
    let copy = copy_verified(staged, parent)?;
    installer_command(copy.path())
        .creation_flags(0x0000_0008 | 0x0000_0200)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(super::install_error)?;
    // The running installer still needs its copy; only the downloaded original goes.
    copy.keep();
    let _ = std::fs::remove_file(&staged.path);
    Ok(super::InstallOutcome::RestartScheduled)
}
