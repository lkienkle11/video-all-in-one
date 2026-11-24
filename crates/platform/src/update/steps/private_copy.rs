//! Private, exclusive copies of a staged file. The caller hashes and uses the copy,
//! never the original, so a swap of the original after verification changes nothing.
use crate::update::install_error;
#[cfg(any(target_os = "macos", target_os = "windows", test))]
use crate::update::verify_staged;
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
#[cfg(any(target_os = "macos", target_os = "windows", test))]
use vaio_core::update::StagedFile;
use vaio_core::update::UpdateError;

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A directory removed on drop unless `disarm` is called.
pub(in crate::update) struct ScopedDir {
    path: PathBuf,
    armed: bool,
}

impl ScopedDir {
    pub fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    #[cfg(any(target_os = "macos", target_os = "windows", test))]
    pub fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ScopedDir {
    fn drop(&mut self) {
        if self.armed {
            // Cleanup is best effort; it must never replace the caller's error.
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// A copy in its own private directory. Dropping it removes the directory.
pub(in crate::update) struct PrivateCopy {
    #[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
    dir: ScopedDir, // removes the directory on drop
    path: PathBuf,
}

impl PrivateCopy {
    pub fn path(&self) -> &Path {
        &self.path
    }
    #[cfg(any(target_os = "windows", test))]
    /// Leaves the copy on disk (a running installer still needs it) and returns its path.
    pub fn keep(mut self) -> PathBuf {
        self.dir.disarm();
        self.path.clone()
    }
}

/// Creates a new directory, mode 0700 on unix. Fails if the path already exists,
/// so an existing directory or symlink is never reused.
pub(in crate::update) fn create_private_dir(path: &Path) -> io::Result<()> {
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

/// Copies `source` (at most `limit` bytes) into a new private directory under `parent`.
pub(in crate::update) fn copy_private(
    source: &Path,
    parent: &Path,
    limit: u64,
) -> Result<PrivateCopy, UpdateError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos());
    let dir = parent.join(format!(
        ".vaio-copy-{}-{}-{stamp}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    create_private_dir(&dir).map_err(install_error)?;
    let dir = ScopedDir::new(dir);
    let name = source.file_name().unwrap_or(std::ffi::OsStr::new("update"));
    let path = dir.path().join(name);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options.open(&path).map_err(install_error)?;
    let mut input = fs::File::open(source)
        .map_err(install_error)?
        .take(limit.saturating_add(1));
    let copied = io::copy(&mut input, &mut output).map_err(install_error)?;
    if copied > limit {
        return Err(install_error("update file is larger than allowed"));
    }
    output.flush().map_err(install_error)?;
    output.sync_all().map_err(install_error)?;
    Ok(PrivateCopy { dir, path })
}

/// Copies the staged file privately, then hashes the copy against `staged`.
#[cfg(any(target_os = "macos", target_os = "windows", test))]
pub(in crate::update) fn copy_verified(
    staged: &StagedFile,
    parent: &Path,
) -> Result<PrivateCopy, UpdateError> {
    let copy = copy_private(&staged.path, parent, staged.size)?;
    verify_staged(&StagedFile {
        path: copy.path().to_path_buf(),
        size: staged.size,
        sha256: staged.sha256,
    })?;
    Ok(copy)
}

/// SHA-256 of the file's bytes.
#[cfg(any(target_os = "linux", test))]
pub(in crate::update) fn sha256_of(path: &Path) -> Result<[u8; 32], UpdateError> {
    use sha2::{Digest, Sha256};
    let mut file = fs::File::open(path).map_err(install_error)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(install_error)?;
        if count == 0 {
            return Ok(digest.finalize().into());
        }
        digest.update(&buffer[..count]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::steps::test_dir::TempDir;
    use sha2::{Digest, Sha256};

    fn staged(dir: &TempDir, body: &[u8]) -> StagedFile {
        let path = dir.0.join("update.zip");
        fs::write(&path, body).unwrap();
        StagedFile {
            path,
            size: body.len() as u64,
            sha256: Sha256::digest(body).into(),
        }
    }

    fn leftovers(dir: &TempDir) -> usize {
        fs::read_dir(&dir.0)
            .unwrap()
            .filter(|entry| {
                entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".vaio-copy-")
            })
            .count()
    }

    #[test]
    fn verified_copy_has_the_same_bytes_in_a_private_directory() {
        let dir = TempDir::new();
        let staged = staged(&dir, b"abc");
        let copy = copy_verified(&staged, &dir.0).unwrap();
        assert_eq!(fs::read(copy.path()).unwrap(), b"abc");
        assert_ne!(copy.path(), staged.path);
        assert_eq!(
            copy.path().parent().unwrap().parent().unwrap(),
            dir.0.as_path()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(copy.path().parent().unwrap()), 0o700);
            assert_eq!(mode(copy.path()), 0o600);
        }
    }

    #[test]
    fn file_changed_after_verification_is_refused_and_leaves_nothing() {
        let dir = TempDir::new();
        let staged = staged(&dir, b"abc");
        crate::update::verify_staged(&staged).unwrap();
        // The attacker rewrites the file between verification and the copy.
        for replacement in [&b"bad"[..], b"ab", b"abcd", b""] {
            fs::write(&staged.path, replacement).unwrap();
            assert!(copy_verified(&staged, &dir.0).is_err());
            assert_eq!(leftovers(&dir), 0);
        }
    }

    #[test]
    fn copy_stops_reading_past_the_limit() {
        let dir = TempDir::new();
        let source = dir.0.join("big");
        fs::write(&source, vec![7_u8; 100]).unwrap();
        assert!(copy_private(&source, &dir.0, 10).is_err());
        assert_eq!(leftovers(&dir), 0);
        assert!(copy_private(&source, &dir.0, 100).is_ok());
    }

    #[test]
    fn dropping_removes_the_copy_and_keep_retains_it() {
        let dir = TempDir::new();
        let staged = staged(&dir, b"abc");
        drop(copy_verified(&staged, &dir.0).unwrap());
        assert_eq!(leftovers(&dir), 0);
        let kept = copy_verified(&staged, &dir.0).unwrap().keep();
        assert_eq!(fs::read(kept).unwrap(), b"abc");
        assert_eq!(leftovers(&dir), 1);
    }

    #[test]
    fn every_copy_gets_its_own_directory() {
        let dir = TempDir::new();
        let staged = staged(&dir, b"abc");
        let first = copy_verified(&staged, &dir.0).unwrap();
        let second = copy_verified(&staged, &dir.0).unwrap();
        assert_ne!(first.path().parent(), second.path().parent());
    }

    #[test]
    fn private_directory_is_never_an_existing_path() {
        let dir = TempDir::new();
        assert!(create_private_dir(&dir.0).is_err());
        let target = dir.0.join("target");
        fs::create_dir(&target).unwrap();
        #[cfg(unix)]
        {
            let link = dir.0.join("link");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            assert!(create_private_dir(&link).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn unwritable_parent_reports_permission_denied() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new();
        fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o500)).unwrap();
        // Running as a superuser would succeed: permissions are not enforced there.
        if let Err(error) = create_private_dir(&dir.0.join("child")) {
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        }
    }

    #[test]
    fn digest_of_a_file_matches_sha256() {
        let dir = TempDir::new();
        let staged = staged(&dir, b"abc");
        assert_eq!(sha256_of(&staged.path).unwrap(), staged.sha256);
        assert!(sha256_of(&dir.0.join("missing")).is_err());
    }
}
