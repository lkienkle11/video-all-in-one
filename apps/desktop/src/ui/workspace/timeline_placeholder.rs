use crate::ui::settings::label;
use gpui_kit::TestSupportExt;
use gpui_kit::{
    component::{ActiveTheme, StyledExt},
    div,
    prelude::*,
    App, IntoElement,
};
use vaio_core::i18n::{tr, Language, Text};

pub(super) fn render(language: Language, cx: &App) -> impl IntoElement {
    div()
        .id("workspace-timeline")
        .test_support()
        .v_flex()
        .size_full()
        .p_3()
        .gap_2()
        .child(label("timeline-title", tr(language, Text::TimelineTitle)).font_semibold())
        .child(
            div()
                .flex_1()
                .min_h_0()
                .bg(cx.theme().muted)
                .p_2()
                .child(label("timeline-empty", tr(language, Text::TimelineEmpty))),
        )
}
