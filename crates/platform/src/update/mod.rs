//! Update installation support: artifact verification for downloaded updates.

mod apply;
mod fallback;
pub mod http;
#[cfg(test)]
mod http_security_tests;
#[cfg(test)]
mod http_tests;
#[cfg(any(target_os = "linux", test))]
mod install_linux;
#[cfg(any(target_os = "macos", test))]
mod install_macos;
#[cfg(test)]
mod install_tests;
#[cfg(any(target_os = "windows", test))]
mod install_windows;
mod steps;
pub mod verify;

pub use apply::handle_apply_update;
use std::path::{Path, PathBuf};
pub use steps::recovery::recover as recover_interrupted_update;
use vaio_core::update::{StagedFile, UpdateError};

#[derive(Debug, PartialEq, Eq)]
pub enum InstallOutcome {
    RestartScheduled,
    OpenedInstaller(PathBuf),
}

/// Installs an artifact already authorized by the core signature verifier.
/// Rechecks its bytes because the file may have changed since download.
pub fn install_for_current_os(
    staged: &StagedFile,
    data_dir: &Path,
) -> Result<InstallOutcome, UpdateError> {
    verify_staged(staged)?;
    #[cfg(target_os = "macos")]
    {
        install_macos::install(staged, data_dir)
    }
    #[cfg(target_os = "windows")]
    {
        let _ = data_dir;
        install_windows::install(staged)
    }
    #[cfg(target_os = "linux")]
    {
        let _ = data_dir;
        install_linux::install(staged)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = data_dir;
        fallback::reveal(&staged.path)
    }
}

fn verify_staged(staged: &StagedFile) -> Result<(), UpdateError> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(&staged.path).map_err(install_error)?;
    let mut size = 0;
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(install_error)?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > staged.size {
            break;
        }
        digest.update(&buffer[..count]);
    }
    if size != staged.size {
        return Err(UpdateError::SizeMismatch {
            expected: staged.size,
            actual: size,
        });
    }
    let actual: [u8; 32] = digest.finalize().into();
    if actual != staged.sha256 {
        return Err(UpdateError::Verify(
            vaio_core::update::VerifyError::DigestMismatch,
        ));
    }
    Ok(())
}

fn install_error(error: impl std::fmt::Display) -> UpdateError {
    UpdateError::Install(error.to_string())
}
