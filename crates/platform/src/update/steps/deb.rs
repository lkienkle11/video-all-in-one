//! The root-side half of a Linux update, run as `pkexec <exe> --apply-deb <path> <sha256>`.
//! It installs only bytes it has copied into a root-owned directory and hashed there.
use super::{hex, private_copy};
use crate::update::{apply::DebRequest, install_error};
use std::{path::Path, process::Command};
use vaio_core::update::{UpdateError, MAX_ASSET_SIZE_BYTES};

/// True for exactly 64 hexadecimal digits.
pub fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// `dpkg -i <copy>` as an argument list, never through a shell.
pub fn dpkg_command(copy: &Path) -> Command {
    // Absolute program: pkexec resets PATH.
    let mut command = Command::new("/usr/bin/dpkg");
    command.arg("-i").arg(copy);
    command
}

/// Copies `request.path` into a new private directory under `base`, hashes the copy,
/// refuses a mismatch, and only then calls `run` with the copy. The copy is removed after.
pub fn install_verified(
    request: &DebRequest,
    base: &Path,
    run: impl FnOnce(&Path) -> Result<(), UpdateError>,
) -> Result<(), UpdateError> {
    if !is_sha256_hex(&request.sha256_hex) {
        return Err(install_error("expected a 64 digit SHA-256"));
    }
    let copy = private_copy::copy_private(&request.path, base, MAX_ASSET_SIZE_BYTES)?;
    // Hash the copy, not the original: only the copy can no longer be swapped.
    let actual = hex(&private_copy::sha256_of(copy.path())?);
    if actual != request.sha256_hex.to_ascii_lowercase() {
        return Err(UpdateError::Verify(
            vaio_core::update::VerifyError::DigestMismatch,
        ));
    }
    run(copy.path())
}

/// Runs the root-side step with the real `dpkg`.
#[cfg(target_os = "linux")]
pub fn apply(request: &DebRequest) -> Result<(), UpdateError> {
    install_verified(request, &std::env::temp_dir(), |copy| {
        let status = dpkg_command(copy).status().map_err(install_error)?;
        if status.success() {
            Ok(())
        } else {
            Err(install_error(format!("dpkg exited with {status}")))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::steps::test_dir::TempDir;
    use sha2::{Digest, Sha256};
    use std::fs;

    fn request(dir: &TempDir, body: &[u8], claimed: &[u8]) -> DebRequest {
        let path = dir.0.join("update.deb");
        fs::write(&path, body).unwrap();
        DebRequest {
            path,
            sha256_hex: hex(&Sha256::digest(claimed)),
        }
    }

    fn entries(dir: &Path) -> usize {
        fs::read_dir(dir).unwrap().count()
    }

    #[test]
    fn hash_argument_must_be_64_hex_digits() {
        assert!(is_sha256_hex(&"a".repeat(64)));
        assert!(is_sha256_hex(&"0123456789abcdefABCDEF".repeat(3)[..64]));
        for bad in [
            "".to_string(),
            "a".repeat(63),
            "a".repeat(65),
            "g".repeat(64),
            format!("{} ", "a".repeat(63)),
            "-".repeat(64),
        ] {
            assert!(!is_sha256_hex(&bad), "{bad:?}");
        }
    }

    #[test]
    fn dpkg_runs_with_an_argument_list_and_no_shell() {
        let command = dpkg_command(Path::new("/root/copy dir/-x; reboot.deb"));
        assert_eq!(command.get_program(), "/usr/bin/dpkg");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["-i", "/root/copy dir/-x; reboot.deb"]
        );
    }

    #[test]
    fn matching_bytes_are_installed_from_a_private_copy_then_removed() {
        let dir = TempDir::new();
        let base = dir.0.join("base");
        fs::create_dir(&base).unwrap();
        let request = request(&dir, b"deb bytes", b"deb bytes");
        let mut seen = None;
        install_verified(&request, &base, |copy| {
            assert!(copy.starts_with(&base));
            assert_ne!(copy, request.path);
            assert_eq!(fs::read(copy).unwrap(), b"deb bytes");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = fs::metadata(copy.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode();
                assert_eq!(mode & 0o777, 0o700);
            }
            seen = Some(copy.to_path_buf());
            Ok(())
        })
        .unwrap();
        assert!(seen.is_some());
        assert_eq!(entries(&base), 0);
    }

    #[test]
    fn mismatching_bytes_are_refused_without_running_dpkg() {
        let dir = TempDir::new();
        let base = dir.0.join("base");
        fs::create_dir(&base).unwrap();
        // The file on disk is not the file whose hash the user approved.
        let request = request(&dir, b"swapped", b"approved");
        let result = install_verified(&request, &base, |_| panic!("must not install"));
        assert!(result.is_err());
        assert_eq!(entries(&base), 0);
    }

    #[test]
    fn installer_failure_is_returned_and_the_copy_still_removed() {
        let dir = TempDir::new();
        let base = dir.0.join("base");
        fs::create_dir(&base).unwrap();
        let request = request(&dir, b"x", b"x");
        let error = install_verified(&request, &base, |_| {
            Err(UpdateError::Install("dpkg failed".into()))
        })
        .unwrap_err();
        assert_eq!(error, UpdateError::Install("dpkg failed".into()));
        assert_eq!(entries(&base), 0);
    }

    #[test]
    fn uppercase_hash_argument_is_accepted() {
        let dir = TempDir::new();
        let base = dir.0.join("base");
        fs::create_dir(&base).unwrap();
        let mut request = request(&dir, b"x", b"x");
        request.sha256_hex = request.sha256_hex.to_uppercase();
        install_verified(&request, &base, |_| Ok(())).unwrap();
    }

    #[test]
    fn missing_source_is_an_error() {
        let dir = TempDir::new();
        let request = DebRequest {
            path: dir.0.join("missing.deb"),
            sha256_hex: "a".repeat(64),
        };
        assert!(install_verified(&request, &dir.0, |_| Ok(())).is_err());
    }
}
