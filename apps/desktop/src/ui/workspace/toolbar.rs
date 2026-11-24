use super::WorkspaceView;
use gpui_kit::TestSupportExt;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        StyledExt,
    },
    div,
    prelude::*,
    Context, IntoElement,
};
use vaio_core::i18n::{tr, Text};

pub(super) fn render(view: &WorkspaceView, cx: &Context<WorkspaceView>) -> impl IntoElement {
    let language = view.language(cx);
    div()
        .id("workspace-toolbar")
        .test_support()
        .h_flex()
        .p_3()
        .gap_2()
        .child(
            Button::new("workspace-add")
                .label(tr(language, Text::WorkspaceAddFile))
                .on_click(cx.listener(|view, _, window, cx| view.prompt_for_source(window, cx))),
        )
        .child(
            Button::new("workspace-settings")
                .ghost()
                .label(tr(language, Text::WorkspaceSettings))
                .on_click(cx.listener(|view, _, window, cx| view.open_settings(window, cx))),
        )
}
