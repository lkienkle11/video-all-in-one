#![cfg(unix)]
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    px, size, AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds,
    WindowOptions,
};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use tokio::{runtime::Runtime, sync::mpsc};
use vaio_core::{
    domain::tool::Tool,
    i18n::{tr, tr_fmt, Language, Text},
    ports::{events::EventSender, JobStore},
    services::job_scheduler::{JobScheduler, Limits},
    settings::Settings,
    update::UpdateState,
};
use vaio_desktop::{
    app::AppView,
    state::{AppState, ToolSearch},
    ui::{
        missing_tools::{check_tools, ensure_tools_with_bundled, install_command},
        settings::SettingsView,
        workspace::WorkspaceView,
    },
};
use vaio_media::{
    export_runner::{FfmpegExportRunner, ToolPathsCell},
    ffmpeg::FfmpegPaths,
};
use vaio_platform::{channel::Channel, settings_store::SettingsStore};
use vaio_storage::{Database, SqliteJobStore};

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

/// The real executables found on `PATH` (the tests link to them, so a plain name is not enough).
fn real_tools() -> FfmpegPaths {
    fn find(name: &str) -> PathBuf {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
            .unwrap_or_else(|| panic!("{name} is not on PATH"))
    }
    FfmpegPaths {
        ffmpeg: find("ffmpeg"),
        ffprobe: find("ffprobe"),
    }
}

fn directory(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "vaio-missing-{}-{}-{name}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A folder holding executable `ffmpeg` and `ffprobe`, linked to the real ones.
fn tool_folder(name: &str) -> PathBuf {
    let dir = directory(name);
    link_tools(&dir);
    dir
}
fn link_tools(dir: &Path) {
    let real = real_tools();
    std::os::unix::fs::symlink(real.ffmpeg, dir.join("ffmpeg")).unwrap();
    std::os::unix::fs::symlink(real.ffprobe, dir.join("ffprobe")).unwrap();
}
fn unlink_tools(dir: &Path) {
    std::fs::remove_file(dir.join("ffmpeg")).unwrap();
    std::fs::remove_file(dir.join("ffprobe")).unwrap();
}

struct Harness {
    handle: AnyWindowHandle,
    state: Entity<AppState>,
    store: Arc<dyn JobStore>,
    dir: PathBuf,
}

/// The search path holds only `search`; the tool cell starts as `initial`.
fn prepare(
    cx: &mut TestAppContext,
    language: Language,
    search: &Path,
    initial: Option<FfmpegPaths>,
) -> (Entity<AppState>, SettingsStore, Arc<dyn JobStore>, PathBuf) {
    cx.update(gpui_kit::init);
    let dir = directory("work");
    let store: Arc<dyn JobStore> = Arc::new(SqliteJobStore::new(
        Database::open(&dir.join("jobs.db")).unwrap(),
    ));
    let cell = ToolPathsCell::new(initial.unwrap_or(FfmpegPaths {
        ffmpeg: PathBuf::new(),
        ffprobe: PathBuf::new(),
    }));
    let runner = Arc::new(FfmpegExportRunner::with_shared_tools(
        store.clone(),
        cell.clone(),
    ));
    let (tx, rx) = mpsc::unbounded_channel();
    let scheduler = {
        let _guard = runtime().enter();
        Arc::new(JobScheduler::new(
            store.clone(),
            runner,
            EventSender(tx),
            Limits {
                downloads: 0,
                encodes: 0,
            },
        ))
    };
    let settings_store = SettingsStore::new(dir.join("settings"));
    settings_store
        .save(&Settings {
            language: Some(language),
            ..Default::default()
        })
        .unwrap();
    let search = ToolSearch::with_path(Some(OsString::from(search)));
    let state = cx.update(|cx| {
        cx.new(|_| {
            AppState::new(runtime().handle().clone(), None, rx)
                .with_scheduler(scheduler)
                .with_tool_search(cell, search)
        })
    });
    (state, settings_store, store, dir)
}

fn options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            Default::default(),
            size(px(1400.), px(1600.)),
        ))),
        ..Default::default()
    }
}

fn open_app(cx: &mut TestAppContext, language: Language, search: &Path) -> Harness {
    let (state, settings_store, store, dir) = prepare(cx, language, search, None);
    let kept = state.clone();
    let (handle, _) = cx.update(|cx| {
        gpui_kit::open_window(options(), cx, |window, cx| {
            cx.new(|cx| {
                AppView::with_store(settings_store, Channel::Development, state, window, cx)
            })
        })
        .unwrap()
    });
    cx.run_until_parked();
    Harness {
        handle,
        state: kept,
        store,
        dir,
    }
}

fn open_workspace(
    cx: &mut TestAppContext,
    search: &Path,
    initial: FfmpegPaths,
) -> (Harness, Entity<WorkspaceView>) {
    let (state, settings_store, store, dir) = prepare(cx, Language::English, search, Some(initial));
    let kept = state.clone();
    let (handle, view) = cx.update(|cx| {
        let settings = cx.new(|cx| {
            SettingsView::new(
                settings_store,
                Channel::Development,
                UpdateState::Idle,
                |_, _, _| {},
                cx,
            )
        });
        gpui_kit::open_window(options(), cx, |window, cx| {
            cx.new(|cx| WorkspaceView::with_audio(state, settings, false, window, cx))
        })
        .unwrap()
    });
    cx.run_until_parked();
    (
        Harness {
            handle,
            state: kept,
            store,
            dir,
        },
        view,
    )
}

fn render(cx: &mut TestAppContext, h: &Harness) {
    cx.update_window(h.handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}
fn click(cx: &mut TestAppContext, h: &Harness, id: &'static str) {
    cx.update_window(h.handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, h);
}
fn type_into(cx: &mut TestAppContext, h: &Harness, id: &'static str, text: &str) {
    cx.update_window(h.handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
        window.press("secondary-a", cx);
        window.input(text, cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, h);
}
fn label_of(cx: &mut TestAppContext, h: &Harness, id: &'static str) -> Option<String> {
    cx.update_window(h.handle, |_, window, cx| {
        window.render_frame(cx);
        window
            .try_find(id)
            .map(|element| element.label().unwrap_or_default().to_owned())
    })
    .unwrap()
}

#[test]
fn check_tools_names_the_first_missing_tool_and_finds_both_in_a_folder() {
    let empty = directory("empty");
    let path = OsString::from(&empty);
    assert_eq!(
        check_tools(None, Some(path.as_os_str())).err(),
        Some(Tool::Ffmpeg)
    );
    let only_ffmpeg = directory("only-ffmpeg");
    std::os::unix::fs::symlink(real_tools().ffmpeg, only_ffmpeg.join("ffmpeg")).unwrap();
    assert_eq!(
        check_tools(Some(&only_ffmpeg), Some(path.as_os_str())).err(),
        Some(Tool::Ffprobe)
    );
    let both = tool_folder("both");
    let found = check_tools(Some(&both), Some(path.as_os_str())).unwrap();
    assert_eq!(found.ffmpeg, both.join("ffmpeg"));
    assert_eq!(found.ffprobe, both.join("ffprobe"));
}

#[test]
fn install_commands_match_the_resolver_hint() {
    for os in ["macos", "windows", "linux"] {
        for tool in [Tool::Ffmpeg, Tool::Ffprobe] {
            let hint = vaio_platform::binary_resolver::install_hint(tool, os);
            assert!(hint.contains(install_command(tool, os)), "{os} {hint}");
        }
    }
}

#[gpui_kit::test]
fn missing_ffmpeg_shows_modal_naming_ffmpeg_in_both_languages(cx: &mut TestAppContext) {
    let search = directory("empty-path");
    for language in [Language::English, Language::Vietnamese] {
        let h = open_app(cx, language, &search);
        render(cx, &h);
        let title = label_of(cx, &h, "modal-title").expect("the missing-tool modal is open");
        assert_eq!(title, tr_fmt(language, Text::MissingToolTitle, &["FFmpeg"]));
        assert!(title.contains("FFmpeg"));
        let body = label_of(cx, &h, "missing-tool-body").unwrap();
        let command = install_command(Tool::Ffmpeg, std::env::consts::OS);
        assert_eq!(
            body,
            tr_fmt(language, Text::MissingToolBody, &["FFmpeg", command])
        );
        assert!(body.contains(command));
        cx.update_window(h.handle, |_, window, cx| {
            assert!(window.find("missing-tool-folder").visible());
            assert!(window.find("missing-tools-check").visible());
            window.press("escape", cx);
        })
        .unwrap();
    }
    assert_ne!(
        tr(Language::English, Text::MissingToolTitle),
        tr(Language::Vietnamese, Text::MissingToolTitle)
    );
}

#[gpui_kit::test]
fn choosing_a_folder_with_the_tool_clears_the_modal(cx: &mut TestAppContext) {
    let search = directory("empty-path");
    let h = open_app(cx, Language::English, &search);
    assert!(h.state.read_with(cx, |state, _| state.tools().is_none()));
    assert!(label_of(cx, &h, "modal-title").is_some());

    // A folder without the tools keeps the modal and says why.
    type_into(
        cx,
        &h,
        "missing-tool-folder",
        directory("bad").to_str().unwrap(),
    );
    click(cx, &h, "missing-tools-check");
    assert!(label_of(cx, &h, "modal-title").is_some());
    assert_eq!(
        label_of(cx, &h, "missing-tool-invalid").unwrap(),
        tr(Language::English, Text::MissingToolInvalid)
    );

    let good = tool_folder("good");
    type_into(cx, &h, "missing-tool-folder", good.to_str().unwrap());
    click(cx, &h, "missing-tools-check");
    assert!(label_of(cx, &h, "modal-title").is_none(), "modal cleared");
    let tools = h.state.read_with(cx, |state, _| state.tools()).unwrap();
    assert_eq!(tools.ffmpeg, good.join("ffmpeg"));
    assert_eq!(tools.ffprobe, good.join("ffprobe"));
    let saved = SettingsStore::load(h.dir.join("settings"));
    assert_eq!(saved.tool_dir, Some(good));
    assert_eq!(saved.language, Some(Language::English));
}

async fn wait_until(cx: &mut TestAppContext, what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if done() {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        runtime()
            .spawn(async { tokio::time::sleep(Duration::from_millis(10)).await })
            .await
            .unwrap();
    }
}

#[gpui_kit::test]
async fn export_is_blocked_while_tool_is_missing(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let search = tool_folder("search");
    let (h, view) = open_workspace(cx, &search, real_tools());
    let source = h.dir.join("clip.mp4");
    let made = runtime()
        .spawn_blocking({
            let source = source.clone();
            move || {
                std::process::Command::new(real_tools().ffmpeg)
                    .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
                    .arg("testsrc2=size=320x240:rate=25")
                    .args(["-t", "4", "-c:v", "libx264", "-preset", "ultrafast"])
                    .arg(&source)
                    .status()
                    .unwrap()
                    .success()
            }
        })
        .await
        .unwrap();
    assert!(made);
    let task = cx
        .update_window(h.handle, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.add_source_path(source.clone(), window, cx)
            })
        })
        .unwrap();
    task.await.unwrap();
    cx.run_until_parked();
    click(cx, &h, "segment-add");
    type_into(cx, &h, "segment-1-start", "1");
    type_into(cx, &h, "segment-1-end", "3");

    // The tool disappears after start: both commands re-check and are blocked.
    unlink_tools(&search);
    for button in ["export-button", "process-all-button"] {
        click(cx, &h, button);
        let title = label_of(cx, &h, "modal-title").expect(button);
        assert_eq!(
            title,
            tr_fmt(Language::English, Text::MissingToolTitle, &["FFmpeg"])
        );
        assert!(h.store.list().unwrap().is_empty(), "{button} queued a job");
        click(cx, &h, "missing-tools-close");
        assert!(label_of(cx, &h, "modal-title").is_none());
    }

    // Once the tool is back the same click goes through.
    link_tools(&search);
    click(cx, &h, "export-button");
    assert!(label_of(cx, &h, "modal-title").is_none());
    wait_until(cx, "the queued job", || !h.store.list().unwrap().is_empty()).await;
    assert_eq!(h.store.list().unwrap().len(), 1);
}

/// Runs the tool check with `bundled` as the bundled folder, in a window that has a settings view.
fn ensure_with_bundle(cx: &mut TestAppContext, h: &Harness, bundled: Option<&Path>) -> bool {
    let store = SettingsStore::new(h.dir.join("settings"));
    cx.update_window(h.handle, |_, window, cx| {
        let settings = cx.new(|cx| {
            SettingsView::new(
                store,
                Channel::Development,
                UpdateState::Idle,
                |_, _, _| {},
                cx,
            )
        });
        ensure_tools_with_bundled(&h.state, &settings, bundled, window, cx)
    })
    .unwrap()
}

#[gpui_kit::test]
fn bundle_present_but_tool_missing_shows_the_reinstall_message(cx: &mut TestAppContext) {
    let search = directory("empty-path");
    let bundle = directory("bundle");
    let (h, _view) = open_workspace(cx, &search, real_tools());
    assert!(!ensure_with_bundle(cx, &h, Some(&bundle)));
    render(cx, &h);
    let title = label_of(cx, &h, "modal-title").expect("the missing-tool modal is open");
    assert_eq!(
        title,
        tr_fmt(Language::English, Text::MissingToolTitle, &["FFmpeg"])
    );
    let body = label_of(cx, &h, "missing-tool-body").unwrap();
    assert_eq!(body, tr(Language::English, Text::MissingToolDamaged));
    assert!(!body.contains(install_command(Tool::Ffmpeg, std::env::consts::OS)));
    cx.update_window(h.handle, |_, window, _| {
        assert!(window.try_find("missing-tool-folder").is_none());
        assert!(window.try_find("missing-tools-check").is_none());
        assert!(window.find("missing-tools-close").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn no_bundle_keeps_the_existing_folder_dialog(cx: &mut TestAppContext) {
    let search = directory("empty-path");
    let (h, _view) = open_workspace(cx, &search, real_tools());
    assert!(!ensure_with_bundle(cx, &h, None));
    render(cx, &h);
    let body = label_of(cx, &h, "missing-tool-body").unwrap();
    let command = install_command(Tool::Ffmpeg, std::env::consts::OS);
    assert_eq!(
        body,
        tr_fmt(
            Language::English,
            Text::MissingToolBody,
            &["FFmpeg", command]
        )
    );
    cx.update_window(h.handle, |_, window, _| {
        assert!(window.find("missing-tool-folder").visible());
        assert!(window.find("missing-tools-check").visible());
    })
    .unwrap();
}
