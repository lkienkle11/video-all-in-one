//! The database file: opening it, checking its schema version and migrating it.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, TransactionBehavior};

use crate::backup::write_backup;
use crate::error::{sqlite, StorageError};
use crate::migrations::{Migration, MIGRATIONS};

/// An open database file, migrated to the supported schema and set up for WAL.
pub struct Database {
    conn: Connection,
    now_ms: fn() -> i64,
}

impl Database {
    /// Opens or creates the database at `path` with the app's migrations and the system clock.
    pub fn open(path: &Path) -> Result<Database, StorageError> {
        Self::open_with(path, MIGRATIONS, system_now_ms)
    }

    /// Opens or creates the database at `path`, applying `migrations` above its schema version.
    /// The supported schema is the highest version in `migrations`. `now_ms` supplies timestamps.
    pub fn open_with(
        path: &Path,
        migrations: &[Migration],
        now_ms: fn() -> i64,
    ) -> Result<Database, StorageError> {
        let mut conn = Connection::open(path).map_err(sqlite)?;
        // The version is read before any other pragma, so a newer file is refused untouched.
        let found: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(sqlite)?;
        let highest = migrations.iter().map(|m| m.version).max().unwrap_or(0);
        if found > highest {
            return Err(StorageError::SchemaTooNew {
                found,
                supported: highest,
            });
        }
        if found == 0 && has_tables(&conn)? {
            return Err(StorageError::ForeignDatabase);
        }
        if found > 0 && found < highest {
            write_backup(&conn, path, found)?;
        }
        apply_migrations(&mut conn, migrations, found)?;
        configure(&conn)?;
        Ok(Database { conn, now_ms })
    }

    /// Gives up the connection and the clock for the job store.
    pub(crate) fn into_parts(self) -> (Connection, fn() -> i64) {
        (self.conn, self.now_ms)
    }

    /// The connection, for tests that check connection settings.
    #[cfg(test)]
    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
}

fn has_tables(conn: &Connection) -> Result<bool, StorageError> {
    let count: i64 = conn
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))
        .map_err(sqlite)?;
    Ok(count > 0)
}

/// Applies each migration above `found` in its own immediate transaction, together with its
/// version marker. A failed migration rolls back and leaves the version unchanged.
fn apply_migrations(
    conn: &mut Connection,
    migrations: &[Migration],
    found: i64,
) -> Result<(), StorageError> {
    let mut pending: Vec<&Migration> = migrations
        .iter()
        .filter(|migration| migration.version > found)
        .collect();
    pending.sort_by_key(|migration| migration.version);
    for migration in pending {
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite)?;
        tx.execute_batch(migration.sql).map_err(sqlite)?;
        tx.pragma_update(None, "user_version", migration.version)
            .map_err(sqlite)?;
        tx.commit().map_err(sqlite)?;
    }
    Ok(())
}

/// Sets the persistent WAL mode and the connection settings, reading back what must hold.
fn configure(conn: &Connection) -> Result<(), StorageError> {
    let mode: String = conn
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(sqlite)?;
    if mode != "wal" {
        return Err(StorageError::Sqlite(format!(
            "journal mode is {mode}, expected wal"
        )));
    }
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000; PRAGMA synchronous = NORMAL;",
    )
    .map_err(sqlite)?;
    let foreign_keys: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .map_err(sqlite)?;
    if foreign_keys != 1 {
        return Err(StorageError::Sqlite(
            "foreign key enforcement is not on".to_string(),
        ));
    }
    Ok(())
}

fn system_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
