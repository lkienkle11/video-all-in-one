//! The window shown when the app data cannot be opened, so the customer sees why the app stopped.

use gpui_kit::base::TestSupportExt;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    ActiveTheme, StyledExt,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, size, App, AppContext, Bounds, Context, IntoElement, Render, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use std::{path::PathBuf, rc::Rc};
use vaio_core::i18n::{tr, tr_fmt, Language, Text};
use vaio_platform::{channel::Channel, settings_store::SettingsStore};
use vaio_storage::StorageError;

const WINDOW_WIDTH_PX: f32 = 480.0;
const WINDOW_HEIGHT_PX: f32 = 260.0;

/// The plain-language reason the app data could not be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupProblem {
    NewerData,
    UnknownDatabase,
    Other,
}

pub fn problem_of(error: &StorageError) -> StartupProblem {
    match error {
        StorageError::SchemaTooNew { .. } => StartupProblem::NewerData,
        StorageError::ForeignDatabase => StartupProblem::UnknownDatabase,
        _ => StartupProblem::Other,
    }
}

/// Why startup stopped, with the data folder when it is known.
pub struct StartupFailure {
    data_folder: Option<PathBuf>,
    database: Option<StorageError>,
    message: String,
}

impl StartupFailure {
    /// The database could not be opened; the storage error is kept to pick the message.
    pub fn database(data_folder: PathBuf, error: StorageError) -> Self {
        Self {
            data_folder: Some(data_folder),
            message: error.to_string(),
            database: Some(error),
        }
    }

    /// Any other startup failure.
    pub fn other(data_folder: Option<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            data_folder,
            database: None,
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn problem(&self) -> StartupProblem {
        self.database
            .as_ref()
            .map_or(StartupProblem::Other, problem_of)
    }

    /// The saved language when the settings file can be read, English otherwise.
    pub fn language(&self) -> Language {
        self.data_folder
            .as_deref()
            .and_then(|folder| SettingsStore::load(folder).language)
            .unwrap_or(Language::English)
    }
}

/// Shows the error window and returns when the customer closes the app.
pub fn show(failure: StartupFailure, channel: Channel) {
    let language = failure.language();
    let problem = failure.problem();
    let folder = failure.data_folder.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(tr(language, Text::AppTitle).into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(WINDOW_WIDTH_PX), px(WINDOW_HEIGHT_PX)),
                ))),
                is_resizable: false,
                app_id: Some(channel.identifier().to_owned()),
                ..Default::default()
            };
            if let Err(error) = gpui_kit::open_window(options, cx, move |_, cx| {
                cx.new(|_| StartupErrorView::new(problem, language, folder, |cx| cx.quit()))
            }) {
                eprintln!("Failed to open the startup error window: {error:#}");
                cx.quit();
            }
        });
}

/// The title, one or two sentences, the data folder and a Close button.
pub struct StartupErrorView {
    language: Language,
    title: SharedString,
    sentence: SharedString,
    folder: Option<SharedString>,
    on_close: Rc<dyn Fn(&mut App)>,
}

impl StartupErrorView {
    pub fn new(
        problem: StartupProblem,
        language: Language,
        data_folder: Option<PathBuf>,
        on_close: impl Fn(&mut App) + 'static,
    ) -> Self {
        let sentence = match problem {
            StartupProblem::NewerData => Text::StartupNewerData,
            StartupProblem::UnknownDatabase => Text::StartupUnknownDatabase,
            StartupProblem::Other => Text::StartupOther,
        };
        Self {
            language,
            title: tr(language, Text::StartupErrorTitle).into(),
            sentence: tr(language, sentence).into(),
            folder: data_folder.map(|folder| {
                tr_fmt(
                    language,
                    Text::StartupDataFolder,
                    &[folder.display().to_string().as_str()],
                )
                .into()
            }),
            on_close: Rc::new(on_close),
        }
    }
}

impl Render for StartupErrorView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let on_close = self.on_close.clone();
        let mut column = div()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .p_6()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .id("startup-error-title")
                    .test_support()
                    .aria_label(self.title.clone())
                    .text_lg()
                    .font_semibold()
                    .child(self.title.clone()),
            )
            .child(
                div()
                    .id("startup-error-sentence")
                    .test_support()
                    .aria_label(self.sentence.clone())
                    .child(self.sentence.clone()),
            );
        if let Some(folder) = &self.folder {
            column = column.child(
                div()
                    .id("startup-error-folder")
                    .test_support()
                    .aria_label(folder.clone())
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(folder.clone()),
            );
        }
        column.child(
            div().flex_1().flex().justify_end().items_end().child(
                Button::new("startup-error-close")
                    .primary()
                    .label(tr(self.language, Text::Close))
                    .on_click(move |_, _, cx| on_close(cx)),
            ),
        )
    }
}
