//! Localized update confirmation. The caller owns all update operations.

use super::modal::{open_with_policy, ModalAction};
use gpui_kit::base::TestSupportExt;
use gpui_kit::prelude::*;
use gpui_kit::{div, App, IntoElement, RenderOnce, Window};
use vaio_core::{
    i18n::{tr, tr_fmt, Language, Text},
    update::AvailableUpdate,
};

pub struct UpdateDialog;

impl UpdateDialog {
    pub fn open(
        language: Language,
        update: AvailableUpdate,
        on_now: impl Fn(&mut Window, &mut App) + 'static,
        on_later: impl Fn(&mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut App,
    ) {
        let later = ModalAction::new(
            "update-later",
            tr(language, Text::UpdateLater),
            move |window, cx| {
                on_later(window, cx);
                true
            },
        );
        open_with_policy(
            window,
            cx,
            tr(language, Text::UpdateConfirmTitle).into(),
            UpdateContent { language, update },
            vec![
                later.clone(),
                ModalAction::new(
                    "update-now",
                    tr(language, Text::UpdateNow),
                    move |window, cx| {
                        on_now(window, cx);
                        true
                    },
                ),
            ],
            Some(later),
        );
    }
}

#[derive(Clone, IntoElement)]
struct UpdateContent {
    language: Language,
    update: AvailableUpdate,
}

impl RenderOnce for UpdateContent {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let size = format!("{:.1}", self.update.size as f64 / 1_000_000.);
        let version = tr_fmt(
            self.language,
            Text::SettingsVersion,
            &[&self.update.version.to_string()],
        );
        let size = tr_fmt(self.language, Text::UpdateConfirmBody, &[&size]);
        let notes = match self.language {
            Language::English => self.update.notes_en,
            Language::Vietnamese => self.update.notes_vi,
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .id("update-version")
                    .test_support()
                    .aria_label(version.clone())
                    .child(version),
            )
            .child(
                div()
                    .id("update-size")
                    .test_support()
                    .aria_label(size.clone())
                    .child(size),
            )
            .child(
                div()
                    .id("update-notes")
                    .test_support()
                    .aria_label(notes.clone())
                    .child(notes),
            )
    }
}
