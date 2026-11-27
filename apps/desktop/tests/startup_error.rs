use gpui_kit::test::TestWindowExt;
use gpui_kit::{px, size, App, AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions};
use std::{cell::Cell, path::PathBuf, rc::Rc};
use vaio_core::i18n::{tr, tr_fmt, Language, Text};
use vaio_desktop::startup_error::{problem_of, StartupErrorView, StartupProblem};
use vaio_storage::StorageError;

#[test]
fn schema_too_new_is_newer_data() {
    let error = StorageError::SchemaTooNew {
        found: 9,
        supported: 3,
    };
    assert_eq!(problem_of(&error), StartupProblem::NewerData);
}

#[test]
fn foreign_database_is_unknown_database() {
    assert_eq!(
        problem_of(&StorageError::ForeignDatabase),
        StartupProblem::UnknownDatabase
    );
}

#[test]
fn other_errors_are_other() {
    for error in [
        StorageError::Sqlite("locked".into()),
        StorageError::Backup("disk full".into()),
        StorageError::Io("denied".into()),
    ] {
        assert_eq!(problem_of(&error), StartupProblem::Other);
    }
}

fn open_error_window(
    cx: &mut TestAppContext,
    folder: PathBuf,
    on_close: impl Fn(&mut App) + 'static,
) -> gpui_kit::AnyWindowHandle {
    cx.update(gpui_kit::init);
    cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(480.), px(260.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, cx| {
                cx.new(|_| {
                    StartupErrorView::new(
                        StartupProblem::NewerData,
                        Language::English,
                        Some(folder),
                        on_close,
                    )
                })
            },
        )
        .unwrap()
    })
    .0
}

#[gpui_kit::test]
fn the_window_shows_the_text_and_the_data_folder(cx: &mut TestAppContext) {
    let folder = PathBuf::from("/data/video-all-in-one/dev");
    let handle = open_error_window(cx, folder.clone(), |_| {});
    let folder_text = tr_fmt(
        Language::English,
        Text::StartupDataFolder,
        &[&folder.display().to_string()],
    );
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("startup-error-title").label(),
            Some(tr(Language::English, Text::StartupErrorTitle))
        );
        assert_eq!(
            window.find("startup-error-sentence").label(),
            Some(tr(Language::English, Text::StartupNewerData))
        );
        assert_eq!(
            window.find("startup-error-folder").label(),
            Some(folder_text.as_str())
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_window_closes_the_app_when_close_is_pressed(cx: &mut TestAppContext) {
    let closed = Rc::new(Cell::new(false));
    let flag = closed.clone();
    let handle = open_error_window(cx, PathBuf::from("/data/video-all-in-one/dev"), move |_| {
        flag.set(true)
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(!closed.get());
        window.click("startup-error-close", cx);
    })
    .unwrap();
    assert!(closed.get());
}
