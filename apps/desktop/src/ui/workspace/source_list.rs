use super::WorkspaceView;
use crate::ui::settings::label;
use gpui_kit::TestSupportExt;
use gpui_kit::{
    component::{button::Button, Selectable, StyledExt},
    div,
    prelude::*,
    Context, IntoElement, SharedString,
};
use vaio_core::i18n::{tr, Text};

pub(super) fn render(view: &WorkspaceView, cx: &Context<WorkspaceView>) -> impl IntoElement {
    let language = view.language(cx);
    let state = view.state.read(cx);
    div()
        .id("workspace-sources")
        .test_support()
        .v_flex()
        .size_full()
        .min_w_0()
        .child(
            label("sources-title", tr(language, Text::SourcesTitle))
                .p_3()
                .font_semibold(),
        )
        .child(
            div()
                .id("source-rows")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_3()
                .when(state.sources().is_empty(), |this| {
                    this.child(label("sources-empty", tr(language, Text::SourcesEmpty)))
                })
                .children(state.sources().iter().map(|source| {
                    let path = source.path().to_owned();
                    let selected = state
                        .selected()
                        .is_some_and(|selected| selected.path() == source.path());
                    let title = format!(
                        "{} · {}",
                        source
                            .path()
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy(),
                        source.info().duration
                    );
                    Button::new(SharedString::from(format!(
                        "source:{}",
                        source.path().display()
                    )))
                    .w_full()
                    .label(title)
                    .selected(selected)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.select_source(path.clone(), window, cx)
                    }))
                })),
        )
}
