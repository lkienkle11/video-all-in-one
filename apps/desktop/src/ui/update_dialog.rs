//! Localized update confirmation. The caller owns all update operations.

use super::modal::{open_with_policy, ModalAction};
use gpui_kit::base::TestSupportExt;
use gpui_kit::component::{button::Button, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::{div, App, IntoElement, RenderOnce, Window};
use std::rc::Rc;
use vaio_core::{
    i18n::{tr, tr_fmt, Language, Text},
    update::AvailableUpdate,
};

pub struct UpdateDialog;

/// When the customer wants the update to be installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateChoice {
    AfterJobs,
    Now,
    Later,
}

type ChoiceCallback = Rc<dyn Fn(UpdateChoice, &mut Window, &mut App)>;

impl UpdateDialog {
    pub fn open(
        language: Language,
        update: AvailableUpdate,
        on_now: impl Fn(&mut Window, &mut App) + 'static,
        on_later: impl Fn(&mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut App,
    ) {
        Self::open_with_jobs(
            language,
            update,
            0,
            move |choice, window, cx| match choice {
                UpdateChoice::Now => on_now(window, cx),
                UpdateChoice::Later | UpdateChoice::AfterJobs => on_later(window, cx),
            },
            window,
            cx,
        );
    }

    pub fn open_with_jobs(
        language: Language,
        update: AvailableUpdate,
        running_jobs: usize,
        on_choice: impl Fn(UpdateChoice, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut App,
    ) {
        let on_choice: ChoiceCallback = Rc::new(on_choice);
        let on_later = on_choice.clone();
        let later = ModalAction::new(
            "update-later",
            tr(language, Text::UpdateLater),
            move |window, cx| {
                on_later(UpdateChoice::Later, window, cx);
                true
            },
        );
        let on_now = on_choice.clone();
        let now = ModalAction::new(
            "update-now",
            tr(
                language,
                if running_jobs == 0 {
                    Text::UpdateNow
                } else {
                    Text::UpdateNowStop
                },
            ),
            move |window, cx| {
                on_now(UpdateChoice::Now, window, cx);
                true
            },
        );
        let actions = if running_jobs == 0 {
            vec![later.clone(), now]
        } else {
            vec![]
        };
        open_with_policy(
            window,
            cx,
            tr(language, Text::UpdateConfirmTitle).into(),
            UpdateContent {
                language,
                update,
                running_jobs,
                on_choice,
            },
            actions,
            Some(later),
        );
    }
}

#[derive(Clone, IntoElement)]
struct UpdateContent {
    language: Language,
    update: AvailableUpdate,
    running_jobs: usize,
    on_choice: ChoiceCallback,
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
            .when(self.running_jobs > 0, |this| {
                let warning = tr_fmt(
                    self.language,
                    Text::UpdateJobsRunning,
                    &[&self.running_jobs.to_string()],
                );
                this.child(
                    div()
                        .id("update-jobs-running")
                        .test_support()
                        .aria_label(warning.clone())
                        .child(warning),
                )
                .children(
                    [
                        (
                            "update-after-jobs",
                            Text::UpdateAfterJobs,
                            UpdateChoice::AfterJobs,
                        ),
                        ("update-now", Text::UpdateNowStop, UpdateChoice::Now),
                        ("update-later", Text::UpdateLater, UpdateChoice::Later),
                    ]
                    .into_iter()
                    .map(|(id, text, choice)| {
                        let on_choice = self.on_choice.clone();
                        Button::new(id).label(tr(self.language, text)).on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                on_choice(choice, window, cx);
                            },
                        )
                    }),
                )
            })
    }
}
