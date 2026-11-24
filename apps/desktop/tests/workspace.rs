use gpui_kit::test::{TestEventExt, TestQueryExt, TestWindowExt};
use gpui_kit::{px, size, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions};
use std::{path::PathBuf, sync::OnceLock};
use tokio::{runtime::Runtime, sync::mpsc};
use vaio_core::{
    domain::job::{JobId, JobState},
    i18n::{tr, Language, Text},
    ports::events::JobEvent,
    settings::Settings,
    update::UpdateState,
};
use vaio_desktop::{
    state::AppState,
    ui::{settings::SettingsView, workspace::WorkspaceView},
};
use vaio_media::ffmpeg::FfmpegPaths;
use vaio_platform::{channel::Channel, settings_store::SettingsStore};

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

fn directory(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vaio-workspace-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn open(
    cx: &mut TestAppContext,
    language: Language,
) -> (
    gpui_kit::AnyWindowHandle,
    Entity<WorkspaceView>,
    mpsc::UnboundedSender<JobEvent>,
) {
    cx.update(gpui_kit::init);
    let (tx, rx) = mpsc::unbounded_channel();
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let store = SettingsStore::new(directory(&format!(
        "settings-{}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )));
    store
        .save(&Settings {
            language: Some(language),
        })
        .unwrap();
    let (window, view) = cx.update(|cx| {
        let state = cx.new(|_| {
            AppState::new(
                runtime().handle().clone(),
                Some(FfmpegPaths {
                    ffmpeg: "/opt/homebrew/bin/ffmpeg".into(),
                    ffprobe: "/opt/homebrew/bin/ffprobe".into(),
                }),
                rx,
            )
        });
        let settings = cx.new(|cx| {
            SettingsView::new(
                store,
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
                    size(px(1200.), px(800.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| WorkspaceView::with_audio(state, settings, false, window, cx)),
        )
        .unwrap()
    });
    cx.run_until_parked();
    (window, view, tx)
}

#[gpui_kit::test]
fn workspace_shows_all_six_regions(cx: &mut TestAppContext) {
    let (handle, _, _) = open(cx, Language::English);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        for id in [
            "workspace-toolbar",
            "workspace-sources",
            "workspace-preview",
            "workspace-inspector",
            "workspace-timeline",
            "workspace-queue",
        ] {
            assert!(window.find(id).visible(), "{id}");
        }
        assert_eq!(
            window.find("preview-empty").label(),
            Some("Add a file to preview")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
async fn adding_a_local_file_lists_it_with_duration(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (handle, view, _) = open(cx, Language::English);
    let path = directory("source").join("nguồn video.mp4");
    let fixture = path.clone();
    runtime()
        .spawn_blocking(move || {
            assert!(std::process::Command::new("/opt/homebrew/bin/ffmpeg")
                .args([
                    "-v",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=size=64x48:rate=10",
                    "-t",
                    "1",
                    "-c:v",
                    "libx264"
                ])
                .arg(fixture)
                .status()
                .unwrap()
                .success());
        })
        .await
        .unwrap();
    let result = cx
        .update_window(handle, |_, window, cx| {
            view.update(cx, |view, cx| view.add_source_path(path, window, cx))
        })
        .unwrap()
        .await;
    assert_eq!(result.unwrap().duration.0, 1000);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.elements().iter().any(|element| {
            element.label().is_some_and(|label| {
                label.contains("nguồn video.mp4") && label.contains("00:01.000")
            })
        }));
        assert!(window.try_find("preview-empty").is_none());
        assert!(window.find("preview-play").visible());
        assert!(window
            .find("inspector-streams")
            .label()
            .unwrap()
            .contains("64 × 48"));
    })
    .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    loop {
        cx.run_until_parked();
        let ready = cx
            .update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                window.find("preview-play").disabled() != Some(true)
                    && window.try_find("preview-status").is_none()
            })
            .unwrap();
        if ready {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "preview did not load: {:?}",
            cx.update_window(handle, |_, window, _| (
                window.find("preview-play").disabled(),
                window
                    .try_find("preview-status")
                    .map(|s| s.label().map(str::to_owned))
            ))
            .unwrap()
        );
        runtime()
            .spawn(async { tokio::time::sleep(std::time::Duration::from_millis(20)).await })
            .await
            .unwrap();
    }
    let target = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let bounds = window.find("preview-seek").bounds();
            let target =
                gpui_kit::point(bounds.origin.x + bounds.size.width * 0.7, bounds.center().y);
            window.pointer_down(target, gpui_kit::MouseButton::Left, cx);
            target
        })
        .unwrap();
    runtime()
        .spawn(async { tokio::time::sleep(std::time::Duration::from_millis(150)).await })
        .await
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.pointer_up(target, gpui_kit::MouseButton::Left, cx);
    })
    .unwrap();
    runtime()
        .spawn(async { tokio::time::sleep(std::time::Duration::from_millis(300)).await })
        .await
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let text = window.find("preview-position");
        let position = vaio_core::domain::time::TimeMs::parse(
            text.label().unwrap().split(" / ").next().unwrap(),
        )
        .unwrap();
        assert!(
            position.0 >= 600,
            "held seek target was overwritten: {position}"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
async fn adding_a_non_media_file_shows_a_localized_error_modal(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    for (language, title, body) in [
        (
            Language::English,
            "Couldn’t open file",
            "Choose a readable video or audio file.",
        ),
        (
            Language::Vietnamese,
            "Không thể mở tệp",
            "Chọn tệp video hoặc âm thanh có thể đọc được.",
        ),
    ] {
        let (handle, view, _) = open(cx, language);
        let path = directory(language.code()).join("invalid.txt");
        std::fs::write(&path, "not media").unwrap();
        assert!(cx
            .update_window(handle, |_, window, cx| view
                .update(cx, |view, cx| view.add_source_path(path, window, cx)))
            .unwrap()
            .await
            .is_err());
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("modal-title").label(), Some(title));
            assert_eq!(window.find("source-error-body").label(), Some(body));
            window.press("escape", cx);
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn job_events_update_the_queue_panel(cx: &mut TestAppContext) {
    let (handle, _, tx) = open(cx, Language::English);
    for event in [
        JobEvent::StateChanged(JobId(7), JobState::Running),
        JobEvent::Progress(JobId(7), 0.5),
        JobEvent::StateChanged(JobId(7), JobState::Failed),
        JobEvent::Failed(JobId(7), "disk full".into()),
    ] {
        tx.send(event).unwrap();
    }
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let row = window.find("job-7");
        let label = row.label().unwrap();
        assert!(
            label.contains("Failed") && label.contains("50%") && label.contains("disk full"),
            "{label}"
        );
    })
    .unwrap();
    tx.send(JobEvent::StateChanged(JobId(7), JobState::Queued))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let row = window.find("job-7");
        let label = row.label().unwrap();
        assert!(
            label.contains("Queued") && label.contains("0%") && !label.contains("disk full"),
            "{label}"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn queue_panel_can_be_collapsed(cx: &mut TestAppContext) {
    let (handle, _, tx) = open(cx, Language::English);
    tx.send(JobEvent::StateChanged(JobId(8), JobState::Queued))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("job-8").visible());
        window.click("queue-toggle", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("job-8").is_none());
        assert!(window.find("workspace-queue").visible());
        window.click("queue-toggle", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("job-8").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn all_workspace_text_changes_with_language(cx: &mut TestAppContext) {
    let (handle, _, tx) = open(cx, Language::English);
    tx.send(JobEvent::StateChanged(JobId(9), JobState::Queued))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("workspace-settings", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("settings-title").label(),
            Some(tr(Language::English, Text::SettingsTitle))
        );
        window.click("settings-vi", cx);
        assert_eq!(window.find("modal-title").label(), Some("Cài đặt"));
        assert_eq!(window.find("settings-close").label(), Some("Đóng"));
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        for (id, text) in [
            ("sources-title", "Nguồn"),
            ("preview-title", "Xem trước"),
            ("inspector-title", "Thông tin"),
            ("timeline-title", "Dòng thời gian"),
            ("preview-empty", "Thêm tệp để xem trước"),
            ("queue-title", "Hàng đợi"),
            ("sources-empty", "Chưa có tệp"),
            ("inspector-empty", "Chọn một tệp để xem thông tin"),
            ("timeline-empty", "Chưa có đoạn video"),
        ] {
            assert_eq!(window.find(id).label(), Some(text), "{id}");
        }
        assert!(window.find("job-9").label().unwrap().contains("Đang chờ"));
    })
    .unwrap();
}
