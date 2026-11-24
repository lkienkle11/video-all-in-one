use gpui_kit::test::{TestEventExt, TestWindowExt};
use gpui_kit::{px, size, AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions};
use std::path::PathBuf;
use vaio_core::i18n::Language;
use vaio_desktop::app::AppView;
use vaio_platform::{channel::Channel, settings_store::SettingsStore};

struct Directory(PathBuf);
impl std::ops::Deref for Directory {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl AsRef<std::path::Path> for Directory {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn directory() -> Directory {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "vaio-language-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Directory(dir)
}

fn open(cx: &mut TestAppContext, dir: &std::path::Path) -> gpui_kit::AnyWindowHandle {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().unwrap());
    cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(800.), px(600.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                let (_, receiver) = tokio::sync::mpsc::unbounded_channel();
                let state = cx.new(|_| {
                    vaio_desktop::state::AppState::new(runtime.handle().clone(), None, receiver)
                });
                cx.new(|cx| {
                    AppView::with_store(
                        SettingsStore::new(dir),
                        Channel::Development,
                        state,
                        window,
                        cx,
                    )
                })
            },
        )
        .unwrap()
        .0
    })
}

#[gpui_kit::test]
fn first_launch_shows_language_modal(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let dir = directory();
    let handle = open(cx, &dir);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("language-modal-body").visible());
        window.press("escape", cx);
        assert!(window.find("language-modal-body").visible());
        let backdrop = gpui_kit::point(px(8.), px(100.));
        window.pointer_down(backdrop, gpui_kit::MouseButton::Left, cx);
        window.pointer_up(backdrop, gpui_kit::MouseButton::Left, cx);
        assert!(window.find("language-modal-body").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn choosing_vietnamese_switches_all_visible_text(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let dir = directory();
    let handle = open(cx, &dir);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("choose-vi", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("language-modal-body").is_none());
        window.click("workspace-settings", cx);
        assert_eq!(window.find("settings-title").label(), Some("Cài đặt"));
        assert_eq!(window.find("language-label").label(), Some("Ngôn ngữ"));
        let version = format!("Phiên bản {}", env!("CARGO_PKG_VERSION"));
        assert_eq!(window.find("version").label(), Some(version.as_str()));
    })
    .unwrap();
}

#[gpui_kit::test]
fn failed_language_save_keeps_modal_open(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let dir = directory();
    std::fs::create_dir(dir.join("settings.json")).unwrap();
    let handle = open(cx, &dir);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("choose-vi", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("language-save-error").visible());
        assert_eq!(
            window.find("language-save-error").label(),
            Some("Couldn’t save your language. Try again.")
        );
        assert!(window.find("language-modal-body").visible());
        assert_eq!(window.find("sources-title").label(), Some("Sources"));
    })
    .unwrap();
    assert!(dir.join("settings.json").is_dir());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    assert_eq!(SettingsStore::load(&dir).language, None);
}

#[gpui_kit::test]
fn choice_is_saved_and_modal_not_shown_again(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let dir = directory();
    let handle = open(cx, &dir);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("choose-vi", cx);
    })
    .unwrap();
    assert_eq!(
        SettingsStore::load(&dir).language,
        Some(Language::Vietnamese)
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("settings.json")).unwrap(),
        "{\"language\":\"vi\"}"
    );
    let reopened = open(cx, &dir);
    cx.run_until_parked();
    cx.update_window(reopened, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("language-modal-body").is_none());
        window.click("workspace-settings", cx);
        assert_eq!(window.find("settings-title").label(), Some("Cài đặt"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn corrupt_settings_file_falls_back_to_language_modal(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let dir = directory();
    std::fs::write(dir.join("settings.json"), "{broken").unwrap();
    let handle = open(cx, &dir);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("language-modal-body").visible());
        window.click("choose-en", cx);
    })
    .unwrap();
    assert_eq!(SettingsStore::load(&dir).language, Some(Language::English));
}
