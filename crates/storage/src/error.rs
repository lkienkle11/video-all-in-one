//! Errors from opening, migrating and backing up the database file.

use std::error::Error;
use std::fmt;

/// Why the database file could not be opened, migrated or backed up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    /// SQLite reported an error; the text is its message.
    Sqlite(String),
    /// The file was written by a newer app. It is refused and left unchanged.
    SchemaTooNew { found: i64, supported: i64 },
    /// The file has tables but no schema version, so it is not one of ours.
    ForeignDatabase,
    /// The backup taken before a migration could not be written.
    Backup(String),
    /// A file system operation failed.
    Io(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Sqlite(message) => write!(f, "database error: {message}"),
            StorageError::SchemaTooNew { found, supported } => write!(
                f,
                "database schema {found} is newer than the supported schema {supported}; the file was not changed"
            ),
            StorageError::ForeignDatabase => write!(
                f,
                "the file is a database with tables but no schema version; the file was not changed"
            ),
            StorageError::Backup(message) => {
                write!(f, "could not back up the database before migrating: {message}")
            }
            StorageError::Io(message) => write!(f, "file error: {message}"),
        }
    }
}

impl Error for StorageError {}

/// Converts a rusqlite error at the crate boundary, so no rusqlite type leaves the crate.
pub(crate) fn sqlite(error: rusqlite::Error) -> StorageError {
    StorageError::Sqlite(error.to_string())
}
