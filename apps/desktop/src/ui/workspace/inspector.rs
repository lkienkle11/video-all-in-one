use super::WorkspaceView;
use crate::ui::settings::label;
use gpui_kit::TestSupportExt;
use gpui_kit::{component::StyledExt, div, prelude::*, Context, IntoElement};
use vaio_core::i18n::{tr, tr_fmt, Text};

pub(super) fn render(view: &WorkspaceView, cx: &Context<WorkspaceView>) -> impl IntoElement {
    let language = view.language(cx);
    let source = view.state.read(cx).selected();
    let details = source.map(|source| {
        let info = source.info();
        let mut lines = vec![tr_fmt(
            language,
            Text::InspectorContainer,
            &[&info.container],
        )];
        if let Some(video) = &info.video {
            lines.push(tr_fmt(
                language,
                Text::InspectorVideo,
                &[
                    &video.codec,
                    &video.width.to_string(),
                    &video.height.to_string(),
                ],
            ));
        }
        for audio in &info.audio {
            lines.push(tr_fmt(
                language,
                Text::InspectorAudio,
                &[&audio.codec, &audio.channels.to_string()],
            ));
        }
        lines.join("\n")
    });
    div()
        .id("workspace-inspector")
        .test_support()
        .v_flex()
        .size_full()
        .min_w_0()
        .child(
            label("inspector-title", tr(language, Text::InspectorTitle))
                .p_3()
                .font_semibold(),
        )
        .child(
            div()
                .id("inspector-content")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_3()
                .v_flex()
                .gap_3()
                .child(match details {
                    Some(text) => label("inspector-streams", text).into_any_element(),
                    None => label("inspector-empty", tr(language, Text::InspectorEmpty))
                        .into_any_element(),
                })
                .child(view.editor.clone())
                .child(view.export.clone()),
        )
}
