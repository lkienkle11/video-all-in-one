//! Exclusive file ownership, publication and rollback for one export.
use super::{file_error, ExecError};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Default)]
pub(super) struct Created {
    paths: Vec<PathBuf>,
}

impl Created {
    pub(super) fn reserve(&mut self, path: &Path) -> Result<fs::File, ExecError> {
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| file_error(path, e))?;
        self.paths.push(path.to_owned());
        Ok(file)
    }
    pub(super) fn write_support(&mut self, path: &Path, contents: &str) -> Result<(), ExecError> {
        self.reserve(path)?
            .write_all(contents.as_bytes())
            .map_err(|e| file_error(path, e))
    }
    pub(super) fn publish(&mut self, temp: &Path, final_path: &Path) -> Result<(), ExecError> {
        rename_exclusive(temp, final_path).map_err(|e| file_error(final_path, e))?;
        self.paths.push(final_path.to_owned());
        Ok(())
    }
    pub(super) fn cleanup(&mut self, keep: &[PathBuf]) -> Result<(), ExecError> {
        for path in self.paths.iter().rev().filter(|p| !keep.contains(p)) {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(file_error(path, error)),
            }
        }
        Ok(())
    }
    pub(super) fn disarm(&mut self) {
        self.paths.clear();
    }
}
impl Drop for Created {
    fn drop(&mut self) {
        let _ = self.cleanup(&[]);
    }
}

// std::fs::rename replaces existing destinations on Unix. These native flags
// atomically refuse a destination created between the plan and publication.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn rename_exclusive(source: &Path, destination: &Path) -> io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let source = CString::new(source.as_os_str().as_bytes())?;
    let destination = CString::new(destination.as_os_str().as_bytes())?;
    // SAFETY: both paths are live NUL-terminated C strings. No pointers escape.
    #[cfg(target_os = "macos")]
    let result =
        unsafe { libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL) };
    // SAFETY: paths are valid C strings and AT_FDCWD resolves absolute paths.
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

// std::fs::rename also replaces an existing destination on Windows (MoveFileEx with
// MOVEFILE_REPLACE_EXISTING), so it must not be used here. A hard link fails when the
// destination exists, which makes the publication exclusive; the temporary name is removed
// afterwards. Source and destination are in the same folder, hence on the same volume.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn rename_exclusive(source: &Path, destination: &Path) -> io::Result<()> {
    fs::hard_link(source, destination)?;
    fs::remove_file(source)
}
