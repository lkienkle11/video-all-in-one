//! Language and update settings.

use gpui_kit::base::TestSupportExt;
use gpui_kit::component::{button::Button, radio::Radio, ActiveTheme, StyledExt};
use gpui_kit::prelude::*;
use gpui_kit::{div, App, Context, IntoElement, Render, SharedString, Window};
use std::rc::Rc;
use vaio_core::{
    i18n::{tr, tr_fmt, Language, Text},
    settings::Settings,
    update::UpdateState,
};
use vaio_platform::{channel::Channel, settings_store::SettingsStore};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateCommand {
    Check,
    Download,
    Install,
}

type UpdateCallback = Rc<dyn Fn(UpdateCommand, &mut Window, &mut App)>;

pub struct SettingsView {
    store: SettingsStore,
    settings: Settings,
    channel: Channel,
    state: UpdateState,
    callback: UpdateCallback,
    save_failed: bool,
    updates_unavailable: bool,
}
impl SettingsView {
    /// Creates a view of caller-supplied update state. No update I/O is started.
    pub fn new(
        store: SettingsStore,
        channel: Channel,
        state: UpdateState,
        callback: impl Fn(UpdateCommand, &mut Window, &mut App) + 'static,
        _cx: &mut Context<Self>,
    ) -> Self {
        let settings = store.read();
        Self {
            store,
            settings,
            channel,
            state,
            callback: Rc::new(callback),
            save_failed: false,
            updates_unavailable: false,
        }
    }
    pub fn set_update_state(&mut self, state: UpdateState, cx: &mut Context<Self>) {
        self.state = state;
        self.updates_unavailable = false;
        cx.notify();
    }
    /// This build cannot update (no public key or manifest URL): say so instead of a state.
    pub fn set_updates_unavailable(&mut self, cx: &mut Context<Self>) {
        self.updates_unavailable = true;
        cx.notify();
    }
    pub(crate) fn needs_language_choice(&self) -> bool {
        self.settings.needs_language_choice()
    }
    pub(crate) fn language(&self) -> Language {
        self.settings.language.unwrap_or(Language::English)
    }
    /// The folder the user chose for FFmpeg and FFprobe.
    pub(crate) fn tool_dir(&self) -> Option<&std::path::Path> {
        self.settings.tool_dir.as_deref()
    }
    pub(crate) fn save_failed(&self) -> bool {
        self.save_failed
    }

    pub(crate) fn choose_language(
        &mut self,
        language: Language,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let settings = Settings {
            language: Some(language),
            ..self.settings.clone()
        };
        self.save_failed = self.store.save(&settings).is_err();
        if !self.save_failed {
            self.settings = settings;
            window.set_window_title(tr(language, Text::AppTitle));
        }
        cx.notify();
        !self.save_failed
    }

    /// Remembers the tool folder; the settings in memory change only when the file was written.
    pub(crate) fn set_tool_dir(&mut self, dir: std::path::PathBuf, cx: &mut Context<Self>) {
        let settings = Settings {
            tool_dir: Some(dir),
            ..self.settings.clone()
        };
        self.save_failed = self.store.save(&settings).is_err();
        if !self.save_failed {
            self.settings = settings;
        }
        cx.notify();
    }

    fn update_section(&self, language: Language) -> impl IntoElement {
        let (status, action) = match &self.state {
            UpdateState::Idle => (
                String::new(),
                Some((Text::UpdateCheck, UpdateCommand::Check)),
            ),
            UpdateState::Checking => (tr(language, Text::UpdateChecking).to_owned(), None),
            UpdateState::UpToDate => (
                tr(language, Text::UpdateUpToDate).to_owned(),
                Some((Text::UpdateCheck, UpdateCommand::Check)),
            ),
            UpdateState::Available(update) => (
                tr_fmt(
                    language,
                    Text::UpdateAvailable,
                    &[&update.version.to_string(), &megabytes(update.size)],
                ),
                Some((Text::UpdateDownload, UpdateCommand::Download)),
            ),
            UpdateState::Downloading {
                done,
                total: Some(total),
            } => (
                tr_fmt(
                    language,
                    Text::UpdateDownloading,
                    &[&megabytes(*done), &megabytes(*total)],
                ),
                None,
            ),
            UpdateState::Downloading { done, total: None } => (
                tr_fmt(
                    language,
                    Text::UpdateDownloadingUnknown,
                    &[&megabytes(*done)],
                ),
                None,
            ),
            UpdateState::Ready(_) => (
                tr(language, Text::UpdateReady).to_owned(),
                Some((Text::UpdateInstall, UpdateCommand::Install)),
            ),
            UpdateState::InstallFailed { .. } => (
                tr(language, Text::UpdateInstallFailed).to_owned(),
                Some((Text::UpdateRetry, UpdateCommand::Install)),
            ),
            UpdateState::Installed => (tr(language, Text::UpdateInstalled).to_owned(), None),
            UpdateState::Failed(_) => (
                tr(language, Text::UpdateFailed).to_owned(),
                Some((Text::UpdateRetry, UpdateCommand::Check)),
            ),
        };
        let status = if self.updates_unavailable {
            tr(language, Text::UpdateNotSetUp).to_owned()
        } else {
            status
        };
        let callback = self.callback.clone();
        div()
            .id("updates")
            .test_support()
            .flex()
            .flex_col()
            .gap_3()
            .child(label("updates-title", tr(language, Text::SettingsUpdates)))
            .when(!status.is_empty(), |this| {
                this.child(label("update-state", status))
            })
            .when_some(action, |this, (text, command)| {
                this.child(
                    Button::new("update-action")
                        .label(tr(language, text))
                        .on_click(move |_, window, cx| callback(command, window, cx)),
                )
            })
    }
}
impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.language();
        div()
            .size_full()
            .flex()
            .flex_col()
            .p_6()
            .gap_6()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                label("settings-title", tr(language, Text::SettingsTitle))
                    .text_xl()
                    .font_semibold(),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(label(
                        "language-label",
                        tr(language, Text::SettingsLanguage),
                    ))
                    .child(
                        Radio::new("settings-en")
                            .label(tr(language, Text::LanguageEnglish))
                            .checked(language == Language::English)
                            .on_change(cx.listener(|this, _, window, cx| {
                                this.choose_language(Language::English, window, cx);
                            })),
                    )
                    .child(
                        Radio::new("settings-vi")
                            .label(tr(language, Text::LanguageVietnamese))
                            .checked(language == Language::Vietnamese)
                            .on_change(cx.listener(|this, _, window, cx| {
                                this.choose_language(Language::Vietnamese, window, cx);
                            })),
                    ),
            )
            .when(self.save_failed, |this| {
                this.child(
                    label("settings-error", tr(language, Text::SettingsSaveFailed))
                        .text_color(cx.theme().danger),
                )
            })
            .child(label(
                "version",
                tr_fmt(
                    language,
                    Text::SettingsVersion,
                    &[env!("CARGO_PKG_VERSION")],
                ),
            ))
            .when(self.channel.updates_enabled(), |this| {
                this.child(self.update_section(language))
            })
    }
}

pub(crate) fn label(id: &'static str, text: impl Into<SharedString>) -> impl IntoElement + Styled {
    let text = text.into();
    div()
        .id(id)
        .test_support()
        .aria_label(text.clone())
        .child(text)
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_000_000.)
}
