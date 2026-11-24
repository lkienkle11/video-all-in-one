//! Process All: one job per configured source, planned before anything is persisted.
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};
use tokio::{runtime::Runtime, sync::mpsc};
use vaio_core::{
    domain::{
        export::{CutMode, ExportMode},
        job::{JobKind, JobState},
        media::MediaInfo,
        time::TimeMs,
    },
    i18n::{tr_fmt, Language, Text},
    ports::{events::EventSender, export_runner::ExportRunner, JobStore},
    services::job_scheduler::{JobScheduler, Limits},
};
use vaio_desktop::{
    commands::{
        batch::{outcome_message, process_all, BatchInput},
        export::ExportCommandError,
    },
    state::SourceDraft,
};
use vaio_media::export_runner::{ExportJobPayload, FfmpegExportRunner};
use vaio_media::ffmpeg::FfmpegPaths;
use vaio_storage::{Database, SqliteJobStore};

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

struct Fixture {
    dir: PathBuf,
    store: Arc<dyn JobStore>,
    scheduler: JobScheduler,
}

/// Nothing may run: both capacity pools are zero, so jobs stay Queued.
fn fixture(name: &str) -> Fixture {
    let dir = std::env::temp_dir().join(format!("vaio-batch-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let store: Arc<dyn JobStore> = Arc::new(SqliteJobStore::new(
        Database::open(&dir.join("jobs.db")).unwrap(),
    ));
    let runner: Arc<dyn ExportRunner> = Arc::new(FfmpegExportRunner::new(
        store.clone(),
        FfmpegPaths {
            ffmpeg: "ffmpeg".into(),
            ffprobe: "ffprobe".into(),
        },
    ));
    let (tx, _rx) = mpsc::unbounded_channel();
    let scheduler = {
        let _guard = runtime().enter();
        JobScheduler::new(
            store.clone(),
            runner,
            EventSender(tx),
            Limits {
                downloads: 0,
                encodes: 0,
            },
        )
    };
    Fixture {
        dir,
        store,
        scheduler,
    }
}

fn info() -> MediaInfo {
    MediaInfo {
        duration: TimeMs(10_000),
        container: "mov,mp4,m4a,3gp,3g2,mj2".into(),
        video: None,
        audio: vec![],
    }
}

fn input(fixture: &Fixture, name: &str, draft: Option<SourceDraft>) -> BatchInput {
    BatchInput {
        path: fixture.dir.join(name),
        info: info(),
        draft,
    }
}

fn rows(rows: &[(&str, &str)]) -> SourceDraft {
    SourceDraft {
        rows: rows
            .iter()
            .map(|(start, end)| (start.to_string(), end.to_string()))
            .collect(),
        ..SourceDraft::default()
    }
}

fn payload(fixture: &Fixture, index: usize) -> (JobKind, JobState, ExportJobPayload) {
    let stored = fixture.store.list().unwrap().remove(index);
    (
        stored.job.kind(),
        stored.job.state(),
        ExportJobPayload::from_json(&stored.payload).unwrap(),
    )
}

#[test]
fn process_all_creates_one_job_per_configured_source() {
    let f = fixture("one-per-source");
    let mut merged = rows(&[("1", "2"), ("nope", "3"), ("4", "5")]);
    merged.mode = ExportMode::Merged;
    merged.cut = CutMode::Precise;
    merged.stem = "joined".into();
    let inputs = [
        input(&f, "a.mp4", Some(rows(&[("0", "3")]))),
        input(&f, "b.mp4", Some(merged)),
        input(&f, "c.mp4", Some(rows(&[("0", "11")]))),
    ];

    let outcome = process_all(&f.scheduler, &inputs).unwrap();

    // c.mp4 has only a row beyond the duration: it counts as having no valid row.
    assert_eq!(outcome.skipped, 1);
    assert_eq!(outcome.jobs.len(), 2);
    assert_eq!(f.store.list().unwrap().len(), 2);
    let (kind, state, first) = payload(&f, 0);
    assert_eq!((kind, state), (JobKind::Cut, JobState::Queued));
    assert_eq!(first.source, f.dir.join("a.mp4"));
    assert_eq!(first.segments, vec![(0, 3_000)]);
    assert_eq!(first.stem, "a");
    assert_eq!(first.out_dir, f.dir.join("a-export"));
    assert!(first.out_dir.is_dir());
    let (kind, _, second) = payload(&f, 1);
    assert_eq!(kind, JobKind::Merge);
    assert_eq!(second.source, f.dir.join("b.mp4"));
    assert_eq!(second.segments, vec![(1_000, 2_000), (4_000, 5_000)]);
    assert_eq!(second.cut, CutMode::Precise);
    assert_eq!(second.stem, "joined");
    assert_eq!(second.out_dir, f.dir.join("b-export"));
    assert!(!f.dir.join("c-export").exists());
}

#[test]
fn sources_without_segments_are_skipped_with_a_message() {
    let f = fixture("skipped");
    let inputs = [
        input(&f, "a.mp4", Some(rows(&[("0", "3")]))),
        input(&f, "b.mp4", None),
        input(&f, "c.mp4", Some(rows(&[]))),
    ];

    let outcome = process_all(&f.scheduler, &inputs).unwrap();

    assert_eq!(outcome.jobs.len(), 1);
    assert_eq!(outcome.skipped, 2);
    assert_eq!(f.store.list().unwrap().len(), 1);
    let english = outcome_message(Language::English, &outcome);
    assert!(english.contains(&tr_fmt(Language::English, Text::ProcessAllSkipped, &["2"])));
    assert!(english.contains(&tr_fmt(Language::English, Text::ProcessAllQueued, &["1"])));
    let vietnamese = outcome_message(Language::Vietnamese, &outcome);
    assert!(vietnamese.contains(&tr_fmt(
        Language::Vietnamese,
        Text::ProcessAllSkipped,
        &["2"]
    )));
    assert_ne!(english, vietnamese);

    let nothing = process_all(&f.scheduler, &inputs[1..]).unwrap();
    assert!(nothing.jobs.is_empty());
    assert_eq!(nothing.skipped, 2);
    assert_eq!(f.store.list().unwrap().len(), 1);
}

#[test]
fn a_source_that_fails_planning_submits_nothing_and_is_named() {
    let f = fixture("fails");
    let mut bad = rows(&[("0", "3")]);
    bad.stem = "a/b".into();
    let inputs = [
        input(&f, "a.mp4", Some(rows(&[("0", "3")]))),
        input(&f, "b.mp4", Some(bad)),
    ];

    let error = process_all(&f.scheduler, &inputs).unwrap_err();

    assert_eq!(error.source, Some(f.dir.join("b.mp4")));
    assert!(matches!(error.error, ExportCommandError::Plan(_)));
    assert!(f.store.list().unwrap().is_empty());
    assert!(!f.dir.join("a-export").exists());
}

#[test]
fn sources_sharing_a_default_folder_get_different_folders() {
    let f = fixture("same-default");
    let inputs = [
        input(&f, "a.mp4", Some(rows(&[("0", "3")]))),
        input(&f, "a.mov", Some(rows(&[("0", "3")]))),
    ];

    process_all(&f.scheduler, &inputs).unwrap();

    let (_, _, first) = payload(&f, 0);
    let (_, _, second) = payload(&f, 1);
    assert_eq!(first.out_dir, f.dir.join("a-export"));
    assert_eq!(second.out_dir, f.dir.join("a-export (2)"));
}
