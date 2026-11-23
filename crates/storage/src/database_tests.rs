//! Tests for opening, migrating and backing up the database file.

use std::fs;
use std::path::Path;

use rusqlite::Connection;
use vaio_core::domain::job::JobKind;
use vaio_core::ports::JobStore;

use crate::backup::append_suffix;
use crate::database::Database;
use crate::error::StorageError;
use crate::job_store::SqliteJobStore;
use crate::migrations::{Migration, MIGRATIONS, SUPPORTED_SCHEMA};
use crate::test_support::{clock_1000, TempDir};

fn user_version(path: &Path) -> i64 {
    let conn = Connection::open(path).expect("open the database file");
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read the schema version")
}

/// The shipped first migration followed by a second one that adds a column.
fn two_versions() -> [Migration; 2] {
    [
        MIGRATIONS[0],
        Migration {
            version: 2,
            sql: "ALTER TABLE jobs ADD COLUMN note TEXT",
        },
    ]
}

/// Creates a schema-1 database at `path` holding one job.
fn seed_v1_with_job(path: &Path) {
    let database = Database::open_with(path, &MIGRATIONS[..1], clock_1000).expect("create v1");
    let store = SqliteJobStore::new(database);
    store.insert(JobKind::Download, "{}").expect("insert a job");
}

#[test]
fn fresh_database_has_schema_version_1() {
    let dir = TempDir::new("fresh");
    Database::open(&dir.database()).expect("open a new database");
    assert_eq!(SUPPORTED_SCHEMA, 1);
    assert_eq!(user_version(&dir.database()), 1);
}

#[test]
fn reopen_keeps_jobs() {
    let dir = TempDir::new("reopen");
    let store = SqliteJobStore::new(
        Database::open_with(&dir.database(), MIGRATIONS, clock_1000).expect("open"),
    );
    let id = store.insert(JobKind::Cut, "{}").expect("insert a job");
    drop(store);

    let store = SqliteJobStore::new(Database::open(&dir.database()).expect("reopen"));
    let jobs = store.list().expect("list the jobs");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job.id(), id);
}

#[test]
fn newer_schema_is_refused_and_untouched() {
    let dir = TempDir::new("newer");
    let path = dir.database();
    {
        let conn = Connection::open(&path).expect("create the file");
        conn.execute_batch("CREATE TABLE notes (a TEXT); PRAGMA user_version = 9;")
            .expect("write a newer schema version");
    }
    let before = fs::read(&path).expect("read the file bytes");

    let err = Database::open(&path)
        .err()
        .expect("a newer schema must be refused");

    assert_eq!(
        err,
        StorageError::SchemaTooNew {
            found: 9,
            supported: 1
        }
    );
    assert_eq!(fs::read(&path).expect("read the file again"), before);
    assert!(!append_suffix(&path, "-wal").exists());
    assert!(!append_suffix(&path, "-shm").exists());
}

#[test]
fn migration_makes_a_backup_first() {
    let dir = TempDir::new("backup-first");
    let path = dir.database();
    seed_v1_with_job(&path);

    Database::open_with(&path, &two_versions(), clock_1000).expect("migrate to v2");

    assert_eq!(user_version(&path), 2);
    let backup = append_suffix(&path, ".bak-v1");
    assert!(backup.exists(), "the backup must exist");
    assert_eq!(user_version(&backup), 1);
    let copy = Connection::open(&backup).expect("open the backup");
    let copied_jobs: i64 = copy
        .query_row("SELECT count(*) FROM jobs", [], |row| row.get(0))
        .expect("count the backed-up jobs");
    assert_eq!(copied_jobs, 1);

    let live = SqliteJobStore::new(
        Database::open_with(&path, &two_versions(), clock_1000).expect("reopen the live file"),
    );
    assert_eq!(live.list().expect("list the live jobs").len(), 1);
}

#[test]
fn backup_name_never_overwrites_an_existing_backup() {
    let dir = TempDir::new("backup-name");
    let path = dir.database();
    seed_v1_with_job(&path);
    let existing = append_suffix(&path, ".bak-v1");
    fs::write(&existing, b"old backup").expect("write an existing backup");

    Database::open_with(&path, &two_versions(), clock_1000).expect("migrate to v2");

    assert_eq!(
        fs::read(&existing).expect("read the old backup"),
        b"old backup"
    );
    assert_eq!(user_version(&append_suffix(&path, ".bak-v1.1")), 1);
}

#[test]
fn failed_migration_rolls_back_and_keeps_the_version() {
    let dir = TempDir::new("failed-migration");
    let path = dir.database();
    seed_v1_with_job(&path);
    let broken = [
        MIGRATIONS[0],
        Migration {
            version: 2,
            sql: "ALTER TABLE no_such_table ADD COLUMN note TEXT",
        },
    ];

    let err = Database::open_with(&path, &broken, clock_1000)
        .err()
        .expect("the broken migration must fail");

    assert!(matches!(err, StorageError::Sqlite(_)));
    assert_eq!(user_version(&path), 1);
    let store = SqliteJobStore::new(
        Database::open_with(&path, &MIGRATIONS[..1], clock_1000).expect("reopen at v1"),
    );
    assert_eq!(store.list().expect("list the jobs").len(), 1);
}

#[test]
fn corrupt_database_file_gives_error_not_panic() {
    let dir = TempDir::new("corrupt");
    let path = dir.database();
    fs::write(&path, vec![b'x'; 4096]).expect("write a garbage file");

    let err = Database::open(&path)
        .err()
        .expect("a garbage file must be refused");

    assert!(matches!(err, StorageError::Sqlite(_)));
}

#[test]
fn foreign_database_without_version_is_refused() {
    let dir = TempDir::new("foreign");
    let path = dir.database();
    {
        let conn = Connection::open(&path).expect("create the file");
        conn.execute_batch("CREATE TABLE notes (a TEXT);")
            .expect("create a foreign table");
    }

    let err = Database::open(&path)
        .err()
        .expect("a foreign database must be refused");

    assert_eq!(err, StorageError::ForeignDatabase);
    assert_eq!(user_version(&path), 0);
}

#[test]
fn wal_mode_is_active_and_foreign_keys_are_on() {
    let dir = TempDir::new("wal");
    let database = Database::open(&dir.database()).expect("open");

    let mode: String = database
        .connection()
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("read the journal mode");
    let foreign_keys: i64 = database
        .connection()
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .expect("read the foreign key setting");

    assert_eq!(mode, "wal");
    assert_eq!(foreign_keys, 1);
}
