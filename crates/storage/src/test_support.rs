//! Helpers for the storage tests: a temporary directory per test and fixed clocks.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

/// A unique temporary directory, removed when the guard is dropped.
pub(crate) struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Creates an empty directory whose name includes `name`, the process id and a counter.
    pub(crate) fn new(name: &str) -> TempDir {
        let number = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vaio-storage-{}-{number}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create the test directory");
        TempDir { path }
    }

    /// The database file path inside this directory.
    pub(crate) fn database(&self) -> PathBuf {
        self.path.join("jobs.sqlite")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// A clock that always reads 1000 ms.
pub(crate) fn clock_1000() -> i64 {
    1_000
}

/// A clock that always reads 2000 ms.
pub(crate) fn clock_2000() -> i64 {
    2_000
}
