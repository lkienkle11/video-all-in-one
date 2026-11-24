//! Rules that the first test file left unpinned: the duration check of `verify`
//! and the rollback of files owned by an execution that is dropped.
use super::files::Created;
use super::tests::Fixture;
use crate::verification::{verify, VerifyFailure};
use std::{fs, time::Duration};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn verify_rejects_a_file_whose_duration_is_wrong() {
    let f = Fixture::new();
    let error = verify(
        &f.source,
        Duration::from_secs(3),
        Duration::from_millis(140),
        &f.tools,
        CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(matches!(error, VerifyFailure::Duration { .. }), "{error:?}");
}

#[test]
fn dropping_created_removes_the_files_it_published() {
    let f = Fixture::new();
    let (temp, published) = (f.dir.join("a.part"), f.dir.join("a.mp4"));
    fs::write(&temp, b"x").unwrap();
    {
        let mut created = Created::default();
        created.publish(&temp, &published).unwrap();
        assert!(published.exists());
    }
    assert!(!published.exists(), "an abandoned export left a final file");
}

#[test]
fn disarmed_created_keeps_the_published_files() {
    let f = Fixture::new();
    let (temp, published) = (f.dir.join("b.part"), f.dir.join("b.mp4"));
    fs::write(&temp, b"x").unwrap();
    {
        let mut created = Created::default();
        created.publish(&temp, &published).unwrap();
        created.disarm();
    }
    assert!(published.exists());
}
