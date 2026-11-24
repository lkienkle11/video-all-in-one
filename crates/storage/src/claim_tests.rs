//! Tests for claiming the next queued job, listing jobs by state, and two connections sharing a file.

use std::path::Path;
use std::thread;

use vaio_core::domain::job::{JobId, JobKind, JobState};
use vaio_core::ports::{JobStore, StoreError};

use crate::database::Database;
use crate::job_store::SqliteJobStore;
use crate::migrations::MIGRATIONS;
use crate::test_support::{clock_1000, clock_2000, TempDir};

fn store_at(path: &Path, clock: fn() -> i64) -> SqliteJobStore {
    SqliteJobStore::new(Database::open_with(path, MIGRATIONS, clock).expect("open the database"))
}

#[test]
fn claim_next_takes_the_oldest_queued_job_and_marks_it_running() {
    let dir = TempDir::new("claim-oldest");
    let store = store_at(&dir.database(), clock_1000);
    let first = store.insert(JobKind::Cut, "a").expect("insert");
    let second = store.insert(JobKind::Merge, "b").expect("insert");

    let claimed = store
        .claim_next()
        .expect("claim")
        .expect("a queued job exists");

    assert_eq!(claimed.job.id(), first);
    assert_eq!(claimed.job.state(), JobState::Running);
    assert_eq!(claimed.job.error(), None);
    assert_eq!(claimed.payload, "a");
    assert_eq!(
        store.get(second).expect("get second").job.state(),
        JobState::Queued
    );
    assert_eq!(
        store.get(first).expect("get first").job.state(),
        JobState::Running
    );
}

#[test]
fn claim_next_skips_non_queued_jobs() {
    let dir = TempDir::new("claim-skip");
    let store = store_at(&dir.database(), clock_1000);
    let running = store.insert(JobKind::Cut, "a").expect("insert");
    let queued = store.insert(JobKind::Cut, "b").expect("insert");
    store
        .transition(running, JobState::Running, None)
        .expect("start the first job");

    let claimed = store
        .claim_next()
        .expect("claim")
        .expect("a queued job exists");

    assert_eq!(claimed.job.id(), queued);
}

#[test]
fn claim_next_returns_none_when_nothing_is_queued() {
    let dir = TempDir::new("claim-none");
    let store = store_at(&dir.database(), clock_1000);
    let id = store.insert(JobKind::Cut, "a").expect("insert");
    store
        .transition(id, JobState::Running, None)
        .expect("start the only job");

    assert_eq!(store.claim_next().expect("claim"), None);
}

#[test]
fn claim_next_updates_the_timestamp() {
    let dir = TempDir::new("claim-timestamp");
    let id = store_at(&dir.database(), clock_1000)
        .insert(JobKind::Cut, "{}")
        .expect("insert");
    let later = store_at(&dir.database(), clock_2000);

    let claimed = later
        .claim_next()
        .expect("claim")
        .expect("a queued job exists");

    assert_eq!(claimed.job.id(), id);
    assert_eq!(claimed.created_at_ms, 1_000);
    assert_eq!(claimed.updated_at_ms, 2_000);
    assert_eq!(later.get(id).expect("get").updated_at_ms, 2_000);
}

#[test]
fn list_by_state_filters_and_orders_by_id() {
    let dir = TempDir::new("list-by-state");
    let store = store_at(&dir.database(), clock_1000);
    let first = store.insert(JobKind::Cut, "a").expect("insert");
    let second = store.insert(JobKind::Cut, "b").expect("insert");
    let third = store.insert(JobKind::Cut, "c").expect("insert");
    store
        .transition(first, JobState::Running, None)
        .expect("start first");
    store
        .transition(third, JobState::Running, None)
        .expect("start third");

    let queued: Vec<JobId> = store
        .list_by_state(JobState::Queued)
        .expect("list queued")
        .iter()
        .map(|stored| stored.job.id())
        .collect();
    let running: Vec<JobId> = store
        .list_by_state(JobState::Running)
        .expect("list running")
        .iter()
        .map(|stored| stored.job.id())
        .collect();

    assert_eq!(queued, vec![second]);
    assert_eq!(running, vec![first, third]);
}

#[test]
fn two_connections_never_claim_the_same_job() {
    let dir = TempDir::new("two-connections");
    let path = dir.database();
    let seed = store_at(&path, clock_1000);
    let total = 40;
    for n in 0..total {
        seed.insert(JobKind::Cut, &n.to_string()).expect("insert");
    }
    drop(seed);

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            thread::spawn(move || {
                let store = store_at(&path, clock_1000);
                let mut claimed = Vec::new();
                while let Some(job) = claim_with_retry(&store) {
                    claimed.push(job.job.id());
                }
                claimed
            })
        })
        .collect();

    let mut ids: Vec<i64> = Vec::new();
    for handle in handles {
        ids.extend(
            handle
                .join()
                .expect("the claim thread finished")
                .iter()
                .map(|id| id.0),
        );
    }
    ids.sort_unstable();
    let unique = {
        let mut copy = ids.clone();
        copy.dedup();
        copy
    };

    assert_eq!(unique.len(), ids.len(), "a job was claimed twice");
    assert_eq!(ids.len(), total);
}

/// Calls `claim_next`, retrying a few times when SQLite reports a lock. The busy timeout should
/// make the retry unnecessary; it is here so a rare lock error does not fail the test.
fn claim_with_retry(store: &SqliteJobStore) -> Option<vaio_core::ports::StoredJob> {
    for _ in 0..5 {
        match store.claim_next() {
            Ok(job) => return job,
            Err(StoreError::Backend(message))
                if message.contains("locked") || message.contains("busy") =>
            {
                thread::yield_now();
            }
            Err(err) => panic!("claim failed: {err}"),
        }
    }
    panic!("claim kept hitting a lock");
}
