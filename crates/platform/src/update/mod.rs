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
pub mod public_key;
mod steps;
pub mod verify;

pub use apply::handle_apply_update;
use std::path::{Path, PathBuf};
pub use steps::recovery::{recover as recover_interrupted_update, ERROR_FILE as UPDATE_ERROR_FILE};
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

/// The manifest URL compiled into this build from `VAIO_UPDATE_MANIFEST_URL`, if any.
pub fn manifest_url() -> Option<url::Url> {
    parse_manifest_url(option_env!("VAIO_UPDATE_MANIFEST_URL"))
}

fn parse_manifest_url(value: Option<&str>) -> Option<url::Url> {
    url::Url::parse(value?.trim()).ok()
}

/// The platform key of the running build, or `None` when no update is published for it.
pub fn platform_key() -> Option<&'static str> {
    key_for(std::env::consts::OS, std::env::consts::ARCH)
}

fn key_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("macos", "aarch64") => Some("macos-aarch64"),
        ("macos", "x86_64") => Some("macos-x86_64"),
        ("windows", "x86_64") => Some("windows-x86_64"),
        ("linux", "x86_64") => Some("linux-x86_64"),
        _ => None,
    }
}

#[cfg(test)]
mod key_tests {
    use super::*;

    #[test]
    fn platform_keys_follow_the_release_names() {
        assert_eq!(key_for("macos", "aarch64"), Some("macos-aarch64"));
        assert_eq!(key_for("macos", "x86_64"), Some("macos-x86_64"));
        assert_eq!(key_for("windows", "x86_64"), Some("windows-x86_64"));
        assert_eq!(key_for("linux", "x86_64"), Some("linux-x86_64"));
        assert_eq!(key_for("linux", "aarch64"), None);
        assert_eq!(key_for("freebsd", "x86_64"), None);
        assert_eq!(key_for("windows", "aarch64"), None);
    }

    #[test]
    fn the_running_build_has_a_known_key_on_supported_hosts() {
        if cfg!(any(
            all(
                target_os = "macos",
                any(target_arch = "aarch64", target_arch = "x86_64")
            ),
            all(
                any(target_os = "windows", target_os = "linux"),
                target_arch = "x86_64"
            )
        )) {
            assert!(platform_key().is_some());
        }
    }

    #[test]
    fn manifest_url_must_be_a_valid_url() {
        assert_eq!(parse_manifest_url(None), None);
        assert_eq!(parse_manifest_url(Some("")), None);
        assert_eq!(parse_manifest_url(Some("not a url")), None);
        assert_eq!(
            parse_manifest_url(Some("https://example.com/latest.json")).map(|url| url.to_string()),
            Some("https://example.com/latest.json".to_owned())
        );
    }
}
