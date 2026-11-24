use gpui_kit::test::TestWindowExt;
use gpui_kit::{px, size, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use tokio::{runtime::Runtime, sync::mpsc, sync::Semaphore};
use vaio_core::{
    domain::{
        export::{CutMode, ExportMode},
        job::{Job, JobKind, JobState},
    },
    i18n::{tr, tr_fmt, Language, Text},
    ports::{
        events::EventSender,
        export_runner::{ExportRunner, RunFuture},
        JobStore,
    },
    services::job_scheduler::{JobScheduler, Limits},
    settings::Settings,
    update::UpdateState,
};
use vaio_desktop::{
    commands::export::default_output_dir,
    state::AppState,
    ui::{settings::SettingsView, workspace::WorkspaceView},
};
use vaio_media::{
    export_runner::{ExportJobPayload, FfmpegExportRunner},
    ffmpeg::FfmpegPaths,
};
use vaio_platform::{channel::Channel, settings_store::SettingsStore};
use vaio_storage::{Database, SqliteJobStore};

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

fn tools() -> FfmpegPaths {
    FfmpegPaths {
        ffmpeg: "ffmpeg".into(),
        ffprobe: "ffprobe".into(),
    }
}

fn directory(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "vaio-cutting-{}-{}-{name}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs the real FFmpeg runner, then holds the job in Running until the test opens the gate.
struct GatedRunner {
    inner: FfmpegExportRunner,
    gate: Arc<Semaphore>,
}
impl ExportRunner for GatedRunner {
    fn run<'a>(
        &'a self,
        job: &'a Job,
        progress: &'a mut (dyn FnMut(f32) + Send),
        cancel: tokio_util::sync::CancellationToken,
    ) -> RunFuture<'a> {
        Box::pin(async move {
            let result = self.inner.run(job, progress, cancel).await;
            self.gate.acquire().await.unwrap().forget();
            result
        })
    }
    fn cleanup_temp(&self, job: &Job) -> Result<(), vaio_core::ports::export_runner::RunFailure> {
        self.inner.cleanup_temp(job)
    }
}

struct Harness {
    handle: gpui_kit::AnyWindowHandle,
    view: Entity<WorkspaceView>,
    store: Arc<dyn JobStore>,
    gate: Arc<Semaphore>,
    dir: PathBuf,
}

fn open(cx: &mut TestAppContext, language: Language, limits: Limits) -> Harness {
    cx.update(gpui_kit::init);
    let dir = directory("work");
    let store: Arc<dyn JobStore> = Arc::new(SqliteJobStore::new(
        Database::open(&dir.join("jobs.db")).unwrap(),
    ));
    let gate = Arc::new(Semaphore::new(0));
    let runner = Arc::new(GatedRunner {
        inner: FfmpegExportRunner::new(store.clone(), tools()),
        gate: gate.clone(),
    });
    let (tx, rx) = mpsc::unbounded_channel();
    let scheduler = {
        let _guard = runtime().enter();
        Arc::new(JobScheduler::new(
            store.clone(),
            runner,
            EventSender(tx),
            limits,
        ))
    };
    let settings_store = SettingsStore::new(dir.join("settings"));
    settings_store
        .save(&Settings {
            language: Some(language),
            ..Default::default()
        })
        .unwrap();
    let (handle, view) = cx.update(|cx| {
        let state = cx.new(|_| {
            AppState::new(runtime().handle().clone(), Some(tools()), rx).with_scheduler(scheduler)
        });
        let settings = cx.new(|cx| {
            SettingsView::new(
                settings_store,
                Channel::Development,
                UpdateState::Idle,
                |_, _, _| {},
                cx,
            )
        });
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(1400.), px(1600.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| WorkspaceView::with_audio(state, settings, false, window, cx)),
        )
        .unwrap()
    });
    cx.run_until_parked();
    Harness {
        handle,
        view,
        store,
        gate,
        dir,
    }
}

async fn make_fixture(path: PathBuf) {
    runtime()
        .spawn_blocking(move || {
            let output = std::process::Command::new(tools().ffmpeg)
                .args([
                    "-v",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=size=320x240:rate=25",
                    "-t",
                    "8",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-pix_fmt",
                    "yuv420p",
                ])
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        })
        .await
        .unwrap();
}

/// Creates the 8 s fixture next to nothing else and adds it as the selected source.
async fn add_source(cx: &mut TestAppContext, harness: &Harness, name: &str) -> PathBuf {
    let path = harness.dir.join(name);
    make_fixture(path.clone()).await;
    let task = cx
        .update_window(harness.handle, |_, window, cx| {
            harness.view.update(cx, |view, cx| {
                view.add_source_path(path.clone(), window, cx)
            })
        })
        .unwrap();
    assert_eq!(task.await.unwrap().duration.0, 8_000);
    cx.run_until_parked();
    path
}

async fn sleep(ms: u64) {
    runtime()
        .spawn(async move { tokio::time::sleep(Duration::from_millis(ms)).await })
        .await
        .unwrap();
}

async fn wait_until(cx: &mut TestAppContext, what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if done() {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        sleep(10).await;
    }
}

fn render(cx: &mut TestAppContext, harness: &Harness) {
    cx.update_window(harness.handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn click(cx: &mut TestAppContext, harness: &Harness, id: &'static str) {
    cx.update_window(harness.handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, harness);
}

fn type_into(cx: &mut TestAppContext, harness: &Harness, id: String, text: &str) {
    cx.update_window(harness.handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
        window.press("secondary-a", cx);
        window.input(text, cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, harness);
}

/// Adds a row (ids count up from 1 in a fresh window) and types its start and end.
fn add_row(cx: &mut TestAppContext, harness: &Harness, row: u32, start: &str, end: &str) {
    click(cx, harness, "segment-add");
    type_into(cx, harness, format!("segment-{row}-start"), start);
    type_into(cx, harness, format!("segment-{row}-end"), end);
}

fn button_disabled(cx: &mut TestAppContext, harness: &Harness, id: &'static str) -> bool {
    cx.update_window(harness.handle, |_, window, cx| {
        window.render_frame(cx);
        window.find(id).disabled() == Some(true)
    })
    .unwrap()
}

fn label_of(cx: &mut TestAppContext, harness: &Harness, id: String) -> Option<String> {
    cx.update_window(harness.handle, |_, window, cx| {
        window.render_frame(cx);
        window
            .try_find(id)
            .map(|element| element.label().unwrap_or_default().to_owned())
    })
    .unwrap()
}

fn payload_of(harness: &Harness, index: usize) -> (Job, ExportJobPayload) {
    let stored = harness.store.list().unwrap().remove(index);
    let payload = ExportJobPayload::from_json(&stored.payload).unwrap();
    (stored.job, payload)
}

fn no_run() -> Limits {
    Limits {
        downloads: 0,
        encodes: 0,
    }
}

#[gpui_kit::test]
async fn valid_segments_create_a_queued_job(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let harness = open(cx, Language::English, no_run());
    let source = add_source(cx, &harness, "clip.mp4").await;
    add_row(cx, &harness, 1, "1", "00:00:03.500");
    assert!(!button_disabled(cx, &harness, "export-button"));
    click(cx, &harness, "export-button");
    wait_until(cx, "the queued job", || {
        !harness.store.list().unwrap().is_empty()
    })
    .await;
    assert_eq!(harness.store.list().unwrap().len(), 1);
    let (job, payload) = payload_of(&harness, 0);
    assert_eq!(job.kind(), JobKind::Cut);
    assert_eq!(job.state(), JobState::Queued);
    assert_eq!(payload.source, source);
    assert_eq!(payload.segments, vec![(1_000, 3_500)]);
    assert_eq!(payload.cut, CutMode::Fast);
    assert_eq!(payload.export, ExportMode::Separate);
    assert_eq!(payload.stem, "clip");
    assert_eq!(payload.out_dir, harness.dir.join("clip-export"));
    assert!(payload.out_dir.is_dir());
    cx.run_until_parked();
    assert_eq!(
        label_of(cx, &harness, "export-status".into()).unwrap(),
        tr_fmt(
            Language::English,
            Text::ExportQueued,
            &[&job.id().0.to_string()]
        )
    );
}

#[gpui_kit::test]
async fn invalid_time_text_marks_the_row_and_blocks_export(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let harness = open(cx, Language::English, no_run());
    add_source(cx, &harness, "clip.mp4").await;
    assert!(button_disabled(cx, &harness, "export-button"), "empty list");
    add_row(cx, &harness, 1, "abc", "3");
    assert_eq!(
        label_of(cx, &harness, "segment-1-error".into()).unwrap(),
        tr(Language::English, Text::SegmentErrorInvalid)
    );
    assert!(button_disabled(cx, &harness, "export-button"));
    type_into(cx, &harness, "segment-1-start".into(), "1");
    assert!(label_of(cx, &harness, "segment-1-error".into()).is_none());
    assert!(!button_disabled(cx, &harness, "export-button"));
    type_into(cx, &harness, "segment-1-end".into(), "9");
    assert_eq!(
        label_of(cx, &harness, "segment-1-error".into()).unwrap(),
        tr_fmt(
            Language::English,
            Text::SegmentErrorBeyond,
            &["00:00:08.000"]
        )
    );
    assert!(button_disabled(cx, &harness, "export-button"));
    click(cx, &harness, "segment-1-remove");
    assert!(label_of(cx, &harness, "segment-1-error".into()).is_none());
    assert!(button_disabled(cx, &harness, "export-button"));
    assert!(harness.store.list().unwrap().is_empty());
}

#[gpui_kit::test]
async fn reversed_segment_shows_localized_error(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    for language in [Language::English, Language::Vietnamese] {
        let harness = open(cx, language, no_run());
        add_source(cx, &harness, "clip.mp4").await;
        add_row(cx, &harness, 1, "5", "2");
        let expected = tr(language, Text::SegmentErrorReversed);
        assert_eq!(
            label_of(cx, &harness, "segment-1-error".into()).unwrap(),
            expected
        );
        assert!(button_disabled(cx, &harness, "export-button"));
        type_into(cx, &harness, "segment-1-end".into(), "5");
        assert_eq!(
            label_of(cx, &harness, "segment-1-error".into()).unwrap(),
            expected,
            "equal start and end"
        );
        assert!(button_disabled(cx, &harness, "export-button"));
    }
}

#[gpui_kit::test]
async fn three_segments_merged_creates_one_merge_job(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let harness = open(cx, Language::English, no_run());
    add_source(cx, &harness, "clip.mp4").await;
    add_row(cx, &harness, 1, "0", "1");
    add_row(cx, &harness, 2, "2", "3");
    add_row(cx, &harness, 3, "4", "5");
    click(cx, &harness, "export-mode-merged");
    click(cx, &harness, "export-button");
    wait_until(cx, "the merge job", || {
        !harness.store.list().unwrap().is_empty()
    })
    .await;
    sleep(100).await;
    assert_eq!(harness.store.list().unwrap().len(), 1);
    let (job, payload) = payload_of(&harness, 0);
    assert_eq!(job.kind(), JobKind::Merge);
    assert_eq!(payload.export, ExportMode::Merged);
    assert_eq!(
        payload.segments,
        vec![(0, 1_000), (2_000, 3_000), (4_000, 5_000)]
    );
}

#[gpui_kit::test]
async fn invalid_name_is_reported_in_the_selected_language(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let harness = open(cx, Language::Vietnamese, no_run());
    add_source(cx, &harness, "clip.mp4").await;
    add_row(cx, &harness, 1, "0", "1");
    type_into(cx, &harness, "export-stem".into(), "a/b");
    click(cx, &harness, "export-button");
    let message = wait_label(cx, &harness, "export-error").await;
    assert_eq!(message, tr(Language::Vietnamese, Text::ExportErrorStem));
    assert!(harness.store.list().unwrap().is_empty());
}

async fn wait_label(cx: &mut TestAppContext, harness: &Harness, id: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        cx.run_until_parked();
        if let Some(label) = label_of(cx, harness, id.to_owned()) {
            return label;
        }
        assert!(Instant::now() < deadline, "no element {id}");
        sleep(10).await;
    }
}

#[gpui_kit::test]
async fn export_does_not_block_the_ui_thread(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let harness = open(cx, Language::English, Limits::default());
    add_source(cx, &harness, "clip.mp4").await;
    add_row(cx, &harness, 1, "0", "6");
    click(cx, &harness, "export-cut-precise");
    click(cx, &harness, "export-button");
    let state_of = |harness: &Harness| harness.store.list().unwrap().first().map(|s| s.job.state());
    wait_until(cx, "the job to run", || {
        state_of(&harness) == Some(JobState::Running)
    })
    .await;
    // The runner holds the job in Running until the gate opens, so this click
    // is handled by the window while the export is genuinely in progress.
    let started = Instant::now();
    click(cx, &harness, "queue-toggle");
    assert!(started.elapsed() < Duration::from_secs(2));
    cx.update_window(harness.handle, |_, window, _| {
        assert!(window.try_find("queue-rows").is_none(), "click not handled");
    })
    .unwrap();
    assert_eq!(state_of(&harness), Some(JobState::Running));
    harness.gate.add_permits(1);
    wait_until(cx, "the job to finish", || {
        state_of(&harness) == Some(JobState::Succeeded)
    })
    .await;
    let (_, payload) = payload_of(&harness, 0);
    assert!(payload.out_dir.join("clip-01.mp4").is_file());
}

#[gpui_kit::test]
async fn output_folder_defaults_next_to_the_source_in_a_new_subfolder_never_over_it(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let harness = open(cx, Language::English, no_run());
    // A file already owns the first candidate name, so the default must pick a free one.
    std::fs::write(harness.dir.join("clip-export"), "occupied").unwrap();
    let source = add_source(cx, &harness, "clip.mp4").await;
    let shown = cx
        .update_window(harness.handle, |_, window, cx| {
            window.render_frame(cx);
            window.find("export-folder").value().map(str::to_owned)
        })
        .unwrap()
        .unwrap();
    let expected = harness.dir.join("clip-export (2)");
    assert_eq!(Path::new(&shown), expected);
    assert_ne!(Path::new(&shown), source.parent().unwrap());
    assert!(!expected.exists());
    let name = cx
        .update_window(harness.handle, |_, window, _| {
            window.find("export-stem").value().map(str::to_owned)
        })
        .unwrap();
    assert_eq!(name.as_deref(), Some("clip"));
    assert_eq!(
        default_output_dir(Path::new("/media/a.mkv"), &|_| false),
        Some(PathBuf::from("/media/a-export"))
    );
    assert_eq!(
        default_output_dir(Path::new("/media/a.mkv"), &|p| p
            == Path::new("/media/a-export")
            || p == Path::new("/media/a-export (2)")),
        Some(PathBuf::from("/media/a-export (3)"))
    );
    assert_eq!(
        default_output_dir(Path::new("relative.mkv"), &|_| false),
        None
    );
}
