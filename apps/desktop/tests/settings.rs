use gpui_kit::test::TestWindowExt;
use gpui_kit::{px, size, AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions};
use vaio_core::{i18n::Language, settings::Settings};
use vaio_desktop::app::AppView;
use vaio_platform::{channel::Channel, settings_store::SettingsStore};

struct Directory(std::path::PathBuf);
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
        let _ = std::fs::remove_file(&self.0);
    }
}
fn directory() -> Directory {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "vaio-settings-{}-{}-{}",
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

fn open(cx: &mut TestAppContext, channel: Channel) -> (gpui_kit::AnyWindowHandle, Directory) {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().unwrap());
    let dir = directory();
    SettingsStore::new(&dir)
        .save(&Settings {
            language: Some(Language::English),
        })
        .unwrap();
    let handle = cx.update(|cx| {
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
                    AppView::with_store(SettingsStore::new(&dir), channel, state, window, cx)
                })
            },
        )
        .unwrap()
        .0
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("workspace-settings", cx);
    })
    .unwrap();
    (handle, dir)
}

#[gpui_kit::test]
fn language_can_be_changed_in_settings(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, dir) = open(cx, Channel::Production);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("settings-vi", cx);
        assert_eq!(window.find("settings-title").label(), Some("Cài đặt"));
        assert_eq!(window.find("language-label").label(), Some("Ngôn ngữ"));
        assert_eq!(
            window.find("update-action").label(),
            Some("Kiểm tra cập nhật")
        );
    })
    .unwrap();
    assert_eq!(
        SettingsStore::load(&dir).language,
        Some(Language::Vietnamese)
    );
}

#[gpui_kit::test]
fn failed_settings_save_shows_error_without_changing_language(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, dir) = open(cx, Channel::Production);
    // Block creation of temporary files without relying on permission bits.
    std::fs::remove_file(dir.join("settings.json")).unwrap();
    std::fs::remove_dir(&dir).unwrap();
    std::fs::write(&dir, "blocked").unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("settings-vi", cx);
        assert!(window.find("settings-error").visible());
        assert_eq!(window.find("settings-title").label(), Some("Settings"));
        assert_eq!(window.find("settings-en").checked(), Some(true));
        assert_ne!(window.find("settings-vi").checked(), Some(true));
    })
    .unwrap();
    assert_eq!(std::fs::read(&dir).unwrap(), b"blocked");
    std::fs::remove_file(&dir).unwrap();
}

#[gpui_kit::test]
fn update_section_hidden_in_development_channel(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (dev, _dev_dir) = open(cx, Channel::Development);
    let (prod, _prod_dir) = open(cx, Channel::Production);
    cx.update_window(dev, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("updates").is_none());
        assert!(window.try_find("update-action").is_none());
    })
    .unwrap();
    cx.update_window(prod, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("updates").visible());
        assert_eq!(
            window.find("update-action").label(),
            Some("Check for updates")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn update_states_show_localized_status_and_dispatch_only_requested_commands(
    cx: &mut TestAppContext,
) {
    update_states(cx, Language::English);
}

#[gpui_kit::test]
fn every_update_state_is_vietnamese(cx: &mut TestAppContext) {
    update_states(cx, Language::Vietnamese);
}

fn update_states(cx: &mut TestAppContext, language: Language) {
    use std::{cell::RefCell, rc::Rc};
    use vaio_core::update::{Asset, AvailableUpdate, StagedFile, UpdateError, UpdateState};
    use vaio_desktop::ui::settings::{SettingsView, UpdateCommand};
    cx.update(gpui_kit::init);
    let dir = directory();
    SettingsStore::new(&dir)
        .save(&Settings {
            language: Some(language),
        })
        .unwrap();
    let commands = Rc::new(RefCell::new(Vec::new()));
    let received = commands.clone();
    let (handle, settings) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(800.), px(600.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, cx| {
                cx.new(|cx| {
                    SettingsView::new(
                        SettingsStore::new(&dir),
                        Channel::Production,
                        UpdateState::Idle,
                        move |command, _, _| received.borrow_mut().push(command),
                        cx,
                    )
                })
            },
        )
        .unwrap()
    });
    let staged = StagedFile {
        path: dir.join("installer"),
        size: 12_500_000,
        sha256: [0; 32],
    };
    let available = AvailableUpdate {
        version: "1.2.3".parse().unwrap(),
        size: 12_500_000,
        notes_en: "Fix".into(),
        notes_vi: "Sửa lỗi".into(),
        asset: Asset {
            url: "https://example.invalid/update".into(),
            size: 12_500_000,
            sha256: "0".repeat(64),
            signature: String::new(),
        },
    };
    let vietnamese = [
        "",
        "Đang kiểm tra cập nhật…",
        "Bạn đang dùng phiên bản mới nhất",
        "Có phiên bản 1.2.3 (12.5 MB)",
        "Đang tải 2.5 trên 12.5 MB",
        "Đang tải 2.5 MB",
        "Sẵn sàng cài đặt",
        "Không thể cài đặt bản cập nhật. Hãy thử lại.",
        "Đã cài đặt bản cập nhật",
        "Không thể cập nhật. Hãy thử lại.",
    ];
    for (index, (state, status, action)) in [
        (UpdateState::Idle, "", Some(UpdateCommand::Check)),
        (UpdateState::Checking, "Checking for updates…", None),
        (
            UpdateState::UpToDate,
            "You have the latest version",
            Some(UpdateCommand::Check),
        ),
        (
            UpdateState::Available(available),
            "Version 1.2.3 is available (12.5 MB)",
            Some(UpdateCommand::Download),
        ),
        (
            UpdateState::Downloading {
                done: 2_500_000,
                total: Some(12_500_000),
            },
            "Downloading 2.5 of 12.5 MB",
            None,
        ),
        (
            UpdateState::Downloading {
                done: 2_500_000,
                total: None,
            },
            "Downloading 2.5 MB",
            None,
        ),
        (
            UpdateState::Ready(staged.clone()),
            "Ready to install",
            Some(UpdateCommand::Install),
        ),
        (
            UpdateState::InstallFailed {
                staged,
                error: UpdateError::Install("technical English".into()),
            },
            "Couldn’t install the update. Try again.",
            Some(UpdateCommand::Install),
        ),
        (UpdateState::Installed, "Update installed", None),
        (
            UpdateState::Failed(UpdateError::Network("technical English".into())),
            "Couldn’t update. Try again.",
            Some(UpdateCommand::Check),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let status = if language == Language::Vietnamese {
            vietnamese[index]
        } else {
            status
        };
        cx.update(|cx| settings.update(cx, |view, cx| view.set_update_state(state, cx)));
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            if status.is_empty() {
                assert!(window.try_find("update-state").is_none());
            } else {
                assert_eq!(window.find("update-state").label(), Some(status));
            }
            let before = commands.borrow().len();
            if let Some(command) = action {
                window.click("update-action", cx);
                assert_eq!(commands.borrow().len(), before + 1);
                assert_eq!(commands.borrow().last(), Some(&command));
            } else {
                assert!(window.try_find("update-action").is_none());
                assert_eq!(commands.borrow().len(), before);
            }
        })
        .unwrap();
    }
}
