//! The backup taken before a schema migration, written through SQLite and never overwriting a file.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::StorageError;

/// Returns `path` with `suffix` appended to its file name.
pub(crate) fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Writes a consistent copy of the open database to a free `<path>.bak-v<found>` name and returns it.
/// `VACUUM INTO` writes through SQLite, so the copy includes committed WAL data.
pub(crate) fn write_backup(
    conn: &Connection,
    path: &Path,
    found: i64,
) -> Result<PathBuf, StorageError> {
    let target = free_name(&append_suffix(path, &format!(".bak-v{found}")));
    let text = target
        .to_str()
        .ok_or_else(|| StorageError::Backup("the backup path is not valid UTF-8".to_string()))?;
    conn.execute("VACUUM INTO ?1", [text])
        .map_err(|err| StorageError::Backup(err.to_string()))?;
    Ok(target)
}

/// The first of `base`, `base.1`, `base.2`, ... that does not exist yet.
fn free_name(base: &Path) -> PathBuf {
    let mut candidate = base.to_path_buf();
    let mut number = 1;
    while candidate.exists() {
        candidate = append_suffix(base, &format!(".{number}"));
        number += 1;
    }
    candidate
}
