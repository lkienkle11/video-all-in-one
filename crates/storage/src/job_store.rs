//! The SQLite implementation of the core's job store port.

use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, TransactionBehavior};
use vaio_core::domain::job::{JobId, JobKind, JobState};
use vaio_core::ports::{JobStore, StoreError, StoredJob};

use crate::database::Database;
use crate::job_rows::{kind_text, state_text, JobRow, SELECT_JOBS};

/// Stores jobs in the database. One connection sits behind a mutex; each call is one transaction
/// (or one statement) on it.
pub struct SqliteJobStore {
    conn: Mutex<Connection>,
    now_ms: fn() -> i64,
}

impl SqliteJobStore {
    /// Takes ownership of an open, migrated database.
    pub fn new(database: Database) -> Self {
        let (conn, now_ms) = database.into_parts();
        Self {
            conn: Mutex::new(conn),
            now_ms,
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.conn
            .lock()
            .map_err(|_| StoreError::Backend("the job store lock is poisoned".to_string()))
    }
}

fn backend(err: rusqlite::Error) -> StoreError {
    StoreError::Backend(err.to_string())
}

/// Reads one job on `conn`, which may be a transaction.
fn load(conn: &Connection, id: JobId) -> Result<StoredJob, StoreError> {
    let sql = format!("{SELECT_JOBS} WHERE id = ?1");
    match conn.query_row(&sql, [id.0], JobRow::read) {
        Ok(row) => row.into_stored(),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(StoreError::NotFound(id)),
        Err(err) => Err(backend(err)),
    }
}

impl JobStore for SqliteJobStore {
    fn insert(&self, kind: JobKind, payload: &str) -> Result<JobId, StoreError> {
        let conn = self.lock()?;
        let now = (self.now_ms)();
        conn.execute(
            "INSERT INTO jobs (kind, state, error, payload, created_at_ms, updated_at_ms) \
             VALUES (?1, ?2, NULL, ?3, ?4, ?4)",
            params![kind_text(kind), state_text(JobState::Queued), payload, now],
        )
        .map_err(backend)?;
        Ok(JobId(conn.last_insert_rowid()))
    }

    fn get(&self, id: JobId) -> Result<StoredJob, StoreError> {
        let conn = self.lock()?;
        load(&conn, id)
    }

    fn list(&self) -> Result<Vec<StoredJob>, StoreError> {
        let conn = self.lock()?;
        let sql = format!("{SELECT_JOBS} ORDER BY id ASC");
        let mut stmt = conn.prepare(&sql).map_err(backend)?;
        let rows = stmt.query_map([], JobRow::read).map_err(backend)?;
        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row.map_err(backend)?.into_stored()?);
        }
        Ok(jobs)
    }

    fn transition(&self, id: JobId, to: JobState, error: Option<&str>) -> Result<(), StoreError> {
        let mut conn = self.lock()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(backend)?;
        let mut job = load(&tx, id)?.job;
        job.transition(to, error.map(String::from))
            .map_err(StoreError::InvalidTransition)?;
        let now = (self.now_ms)();
        tx.execute(
            "UPDATE jobs SET state = ?1, error = ?2, updated_at_ms = ?3 WHERE id = ?4",
            params![state_text(job.state()), job.error(), now, id.0],
        )
        .map_err(backend)?;
        tx.commit().map_err(backend)
    }

    fn recover_interrupted(&self) -> Result<Vec<JobId>, StoreError> {
        let mut conn = self.lock()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(backend)?;
        let ids = {
            let mut stmt = tx
                .prepare("SELECT id FROM jobs WHERE state = ?1 ORDER BY id ASC")
                .map_err(backend)?;
            let rows = stmt
                .query_map([state_text(JobState::Running)], |row| row.get::<_, i64>(0))
                .map_err(backend)?;
            let mut ids = Vec::new();
            for row in rows {
                ids.push(row.map_err(backend)?);
            }
            ids
        };
        let now = (self.now_ms)();
        tx.execute(
            "UPDATE jobs SET state = ?1, error = ?2, updated_at_ms = ?3 WHERE state = ?4",
            params![
                state_text(JobState::Failed),
                "interrupted",
                now,
                state_text(JobState::Running)
            ],
        )
        .map_err(backend)?;
        tx.commit().map_err(backend)?;
        Ok(ids.into_iter().map(JobId).collect())
    }
}
