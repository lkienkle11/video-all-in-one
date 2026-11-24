use super::WorkspaceView;
use crate::ui::settings::label;
use gpui_kit::TestSupportExt;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        StyledExt,
    },
    div,
    prelude::*,
    Context, IntoElement, SharedString,
};
use vaio_core::{
    domain::job::JobState,
    i18n::{tr, Text},
};

pub(super) fn render(view: &WorkspaceView, cx: &Context<WorkspaceView>) -> impl IntoElement {
    let language = view.language(cx);
    let jobs = view.state.read(cx).jobs();
    div()
        .id("workspace-queue")
        .test_support()
        .v_flex()
        .size_full()
        .min_h_0()
        .child(
            div()
                .h_flex()
                .p_2()
                .gap_3()
                .child(label("queue-title", tr(language, Text::QueueTitle)).font_semibold())
                .child(
                    Button::new("queue-toggle")
                        .ghost()
                        .label(tr(
                            language,
                            if view.queue_collapsed {
                                Text::QueueExpand
                            } else {
                                Text::QueueCollapse
                            },
                        ))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.queue_collapsed = !view.queue_collapsed;
                            cx.notify();
                        })),
                ),
        )
        .when(!view.queue_collapsed, |this| {
            this.child(
                div()
                    .id("queue-rows")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_3()
                    .when(jobs.is_empty(), |this| {
                        this.child(label("queue-empty", tr(language, Text::QueueEmpty)))
                    })
                    .children(jobs.iter().map(|row| {
                        let key = match row.state() {
                            Some(JobState::Queued) => Text::JobQueued,
                            Some(JobState::Running) => Text::JobRunning,
                            Some(JobState::Succeeded) => Text::JobSucceeded,
                            Some(JobState::Failed) => Text::JobFailed,
                            Some(JobState::Cancelled) => Text::JobCancelled,
                            None => Text::JobUnknown,
                        };
                        let text = format!(
                            "#{} · {} · {:.0}%{}",
                            row.id().0,
                            tr(language, key),
                            row.progress() * 100.,
                            row.error()
                                .map(|error| format!(" · {error}"))
                                .unwrap_or_default()
                        );
                        div()
                            .id(SharedString::from(format!("job-{}", row.id().0)))
                            .test_support()
                            .aria_label(text.clone())
                            .child(text)
                    })),
            )
        })
}
