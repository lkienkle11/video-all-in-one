//! First-launch language choice.

use super::{
    modal::{open_with_policy, ModalAction},
    settings::{label, SettingsView},
};
use gpui_kit::prelude::*;
use gpui_kit::{div, App, AppContext, Context, Entity, IntoElement, Render, Subscription, Window};
use vaio_core::i18n::{tr, Language, Text};

pub struct LanguageModal {
    settings: Entity<SettingsView>,
    _subscription: Subscription,
}

impl LanguageModal {
    pub fn open(settings: Entity<SettingsView>, window: &mut Window, cx: &mut App) {
        let language = settings.read(cx).language();
        let body = cx.new(|cx| Self {
            _subscription: cx.observe(&settings, |_, _, cx| cx.notify()),
            settings: settings.clone(),
        });
        let actions = [
            ("choose-en", Language::English, Text::LanguageEnglish),
            ("choose-vi", Language::Vietnamese, Text::LanguageVietnamese),
        ]
        .into_iter()
        .map(|(id, choice, text)| {
            let settings = settings.clone();
            ModalAction::new(id, tr(language, text), move |window, cx| {
                settings.update(cx, |view, cx| view.choose_language(choice, window, cx))
            })
        })
        .collect();
        open_with_policy(
            window,
            cx,
            tr(language, Text::ChooseLanguageTitle).into(),
            body,
            actions,
            None,
        );
    }
}

impl Render for LanguageModal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = self.settings.read(cx);
        let language = settings.language();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(label(
                "language-modal-body",
                tr(language, Text::ChooseLanguageBody),
            ))
            .when(settings.save_failed(), |this| {
                this.child(label(
                    "language-save-error",
                    tr(language, Text::SettingsSaveFailed),
                ))
            })
    }
}
