//! Tests for the SQLite job store.

use std::path::Path;

use rusqlite::Connection;
use vaio_core::domain::job::{InvalidTransition, JobId, JobKind, JobState};
use vaio_core::ports::{JobStore, StoreError};

use crate::database::Database;
use crate::job_store::SqliteJobStore;
use crate::migrations::MIGRATIONS;
use crate::test_support::{clock_1000, clock_2000, TempDir};

fn store_at(path: &Path, clock: fn() -> i64) -> SqliteJobStore {
    SqliteJobStore::new(Database::open_with(path, MIGRATIONS, clock).expect("open the database"))
}

#[test]
fn job_is_persisted_before_it_can_run() {
    let dir = TempDir::new("persisted");
    let id = store_at(&dir.database(), clock_1000)
        .insert(JobKind::Cut, "{}")
        .expect("insert a job");

    let reopened = SqliteJobStore::new(Database::open(&dir.database()).expect("reopen"));

    assert_eq!(
        reopened.get(id).expect("get the job").job.state(),
        JobState::Queued
    );
}

#[test]
fn invalid_transition_is_rejected_and_state_unchanged() {
    let dir = TempDir::new("invalid");
    let store = store_at(&dir.database(), clock_1000);
    let id = store.insert(JobKind::Cut, "{}").expect("insert a job");

    let err = store
        .transition(id, JobState::Succeeded, None)
        .expect_err("a queued job cannot succeed");

    assert_eq!(
        err,
        StoreError::InvalidTransition(InvalidTransition {
            from: JobState::Queued,
            to: JobState::Succeeded
        })
    );
    assert_eq!(
        store.get(id).expect("get the job").job.state(),
        JobState::Queued
    );
}

#[test]
fn recover_interrupted_marks_running_as_failed() {
    let dir = TempDir::new("recover");
    let store = store_at(&dir.database(), clock_1000);
    let first = store.insert(JobKind::Cut, "{}").expect("insert");
    let second = store.insert(JobKind::Merge, "{}").expect("insert");
    let third = store.insert(JobKind::Download, "{}").expect("insert");
    store
        .transition(first, JobState::Running, None)
        .expect("start first");
    store
        .transition(third, JobState::Running, None)
        .expect("start third");

    let recovered = store.recover_interrupted().expect("recover");

    assert_eq!(recovered, vec![first, third]);
    let job = store.get(first).expect("get first").job;
    assert_eq!(job.state(), JobState::Failed);
    assert_eq!(job.error(), Some("interrupted"));
    assert_eq!(
        store.get(second).expect("get second").job.state(),
        JobState::Queued
    );
}

#[test]
fn recover_with_no_running_jobs_returns_empty() {
    let dir = TempDir::new("recover-empty");
    let store = store_at(&dir.database(), clock_1000);
    store.insert(JobKind::Cut, "{}").expect("insert");

    assert_eq!(
        store.recover_interrupted().expect("recover"),
        Vec::<JobId>::new()
    );
}

#[test]
fn list_is_ordered_by_id() {
    let dir = TempDir::new("order");
    let store = store_at(&dir.database(), clock_1000);
    let first = store.insert(JobKind::Cut, "a").expect("insert");
    let second = store.insert(JobKind::Merge, "b").expect("insert");
    let third = store.insert(JobKind::Download, "c").expect("insert");

    let ids: Vec<JobId> = store
        .list()
        .expect("list")
        .iter()
        .map(|stored| stored.job.id())
        .collect();

    assert_eq!(ids, vec![first, second, third]);
}

#[test]
fn timestamps_use_the_injected_clock() {
    let dir = TempDir::new("clock");
    let store = store_at(&dir.database(), clock_1000);
    let id = store.insert(JobKind::Cut, "{}").expect("insert");

    let stored = store.get(id).expect("get");

    assert_eq!(stored.created_at_ms, 1_000);
    assert_eq!(stored.updated_at_ms, 1_000);
}

#[test]
fn transition_updates_updated_at() {
    let dir = TempDir::new("updated-at");
    let id = store_at(&dir.database(), clock_1000)
        .insert(JobKind::Cut, "{}")
        .expect("insert");
    let later = store_at(&dir.database(), clock_2000);

    later
        .transition(id, JobState::Running, None)
        .expect("start the job");

    let stored = later.get(id).expect("get");
    assert_eq!(stored.created_at_ms, 1_000);
    assert_eq!(stored.updated_at_ms, 2_000);
}

#[test]
fn retry_clears_the_error() {
    let dir = TempDir::new("retry");
    let store = store_at(&dir.database(), clock_1000);
    let id = store.insert(JobKind::Cut, "{}").expect("insert");
    store
        .transition(id, JobState::Running, None)
        .expect("start");
    store
        .transition(id, JobState::Failed, Some("disk full"))
        .expect("fail");

    store.transition(id, JobState::Queued, None).expect("retry");

    let job = store.get(id).expect("get").job;
    assert_eq!(job.state(), JobState::Queued);
    assert_eq!(job.error(), None);
}

#[test]
fn failed_transition_stores_the_error_text() {
    let dir = TempDir::new("error-text");
    let store = store_at(&dir.database(), clock_1000);
    let id = store.insert(JobKind::Cut, "{}").expect("insert");
    store
        .transition(id, JobState::Running, None)
        .expect("start");

    store
        .transition(id, JobState::Failed, Some("disk full"))
        .expect("fail");

    assert_eq!(store.get(id).expect("get").job.error(), Some("disk full"));
}

#[test]
fn not_found_id_is_an_error() {
    let dir = TempDir::new("not-found");
    let store = store_at(&dir.database(), clock_1000);
    let missing = JobId(999);

    assert_eq!(
        store.get(missing).expect_err("no such job"),
        StoreError::NotFound(missing)
    );
    assert_eq!(
        store
            .transition(missing, JobState::Running, None)
            .expect_err("no such job"),
        StoreError::NotFound(missing)
    );
}

#[test]
fn sql_injection_in_the_payload_is_stored_as_text() {
    let dir = TempDir::new("injection");
    let store = store_at(&dir.database(), clock_1000);
    let payload = "'); DROP TABLE jobs; --";
    let id = store.insert(JobKind::Cut, payload).expect("insert");

    assert_eq!(store.get(id).expect("get").payload, payload);
    assert_eq!(store.list().expect("list").len(), 1);
}

#[test]
fn ids_are_not_reused_after_deleting_the_last_row() {
    let dir = TempDir::new("ids-not-reused");
    let store = store_at(&dir.database(), clock_1000);
    store
        .insert(JobKind::Cut, "{}")
        .expect("insert the first job");
    let deleted = store
        .insert(JobKind::Cut, "{}")
        .expect("insert the second job");
    {
        let conn = Connection::open(dir.database()).expect("open a second connection");
        conn.execute("DELETE FROM jobs WHERE id = ?1", [deleted.0])
            .expect("delete the highest id");
    }

    let next = store.insert(JobKind::Cut, "{}").expect("insert again");

    assert!(next.0 > deleted.0, "id {} was reused", next.0);
}

#[test]
fn one_megabyte_payload_round_trips() {
    let dir = TempDir::new("big-payload");
    let store = store_at(&dir.database(), clock_1000);
    let payload = "x".repeat(1_000_000);
    let id = store.insert(JobKind::Download, &payload).expect("insert");

    assert_eq!(store.get(id).expect("get").payload, payload);
}

#[test]
fn unknown_stored_state_is_a_backend_error() {
    let dir = TempDir::new("unknown-state");
    let store = store_at(&dir.database(), clock_1000);
    let id = store.insert(JobKind::Cut, "{}").expect("insert");
    {
        let conn = Connection::open(dir.database()).expect("open a raw connection");
        conn.execute_batch("PRAGMA ignore_check_constraints = ON;")
            .expect("relax the check constraints");
        conn.execute("UPDATE jobs SET state = 'bogus' WHERE id = ?1", [id.0])
            .expect("write an unknown state");
    }

    let err = store
        .get(id)
        .expect_err("an unknown state must be an error");

    assert!(matches!(err, StoreError::Backend(ref message) if message.contains("bogus")));
}

#[test]
fn submit_many_is_all_or_nothing() {
    let dir = TempDir::new("insert-many");
    let store = store_at(&dir.database(), clock_1000);
    let before = store.insert(JobKind::Cut, "before").expect("insert");

    let ids = store
        .insert_many(&[(JobKind::Merge, "a"), (JobKind::Cut, "b")])
        .expect("insert both");
    assert_eq!(ids.len(), 2);
    assert!(before.0 < ids[0].0 && ids[0].0 < ids[1].0);
    assert_eq!(store.get(ids[0]).expect("get a").payload, "a");
    assert_eq!(store.get(ids[1]).expect("get b").job.kind(), JobKind::Cut);
    assert_eq!(store.insert_many(&[]).expect("empty batch"), vec![]);

    // A constraint that refuses the third row, as a full disk or a lock would mid-batch.
    Connection::open(dir.database())
        .expect("open a second connection")
        .execute_batch(
            "CREATE TRIGGER refuse_poison BEFORE INSERT ON jobs WHEN NEW.payload = 'poison' \
             BEGIN SELECT RAISE(ABORT, 'poison refused'); END;",
        )
        .expect("create the trigger");
    let rows_before = store.list().expect("list").len();

    let error = store
        .insert_many(&[
            (JobKind::Cut, "x"),
            (JobKind::Cut, "y"),
            (JobKind::Cut, "poison"),
        ])
        .expect_err("the third row is refused");

    assert!(error.to_string().contains("poison refused"));
    assert_eq!(store.list().expect("list").len(), rows_before);
    let next = store.insert(JobKind::Cut, "after").expect("insert");
    assert_eq!(next.0, ids[1].0 + 1, "a rolled-back batch uses no ids");
}
