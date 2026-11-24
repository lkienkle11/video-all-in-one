//! Typed start and end rows for the segments to export, validated as the user types.
use crate::{state::AppState, ui::settings::SettingsView};
use gpui_kit::TestSupportExt;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        ActiveTheme, Disableable, StyledExt,
    },
    div,
    prelude::*,
    App, AppContext, Context, Entity, IntoElement, Render, SharedString, Subscription, Window,
};
use std::path::PathBuf;
use vaio_core::{
    domain::{segment::Segment, time::TimeMs},
    i18n::{tr, tr_fmt, Language, Text},
};

/// Why a row cannot be exported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowError {
    Invalid,
    NotIncreasing,
    BeyondDuration,
}

fn check_row(start: &str, end: &str, duration: TimeMs) -> Result<Segment, RowError> {
    let (Ok(start), Ok(end)) = (TimeMs::parse(start), TimeMs::parse(end)) else {
        return Err(RowError::Invalid);
    };
    if start >= end {
        return Err(RowError::NotIncreasing);
    }
    if end > duration {
        return Err(RowError::BeyondDuration);
    }
    Ok(Segment::new(start, end))
}

fn error_text(error: RowError, duration: TimeMs, language: Language) -> String {
    match error {
        RowError::Invalid => tr(language, Text::SegmentErrorInvalid).to_owned(),
        RowError::NotIncreasing => tr(language, Text::SegmentErrorReversed).to_owned(),
        RowError::BeyondDuration => {
            tr_fmt(language, Text::SegmentErrorBeyond, &[&duration.to_string()])
        }
    }
}

struct Row {
    id: u64,
    start: Entity<InputState>,
    end: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

pub(super) struct SegmentEditor {
    state: Entity<AppState>,
    settings: Entity<SettingsView>,
    rows: Vec<Row>,
    next_id: u64,
    source: Option<PathBuf>,
    _subscriptions: Vec<Subscription>,
}
impl SegmentEditor {
    pub(super) fn new(
        state: Entity<AppState>,
        settings: Entity<SettingsView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let source = selected_path(&state, cx);
        let subscriptions = vec![
            cx.observe_in(&state, window, |this, state, _, cx| {
                // Rows belong to one source; another source starts with an empty list.
                let selected = selected_path(&state, cx);
                if selected != this.source {
                    this.source = selected;
                    this.rows.clear();
                }
                cx.notify();
            }),
            cx.observe(&settings, |_, _, cx| cx.notify()),
        ];
        Self {
            state,
            settings,
            rows: vec![],
            next_id: 1,
            source,
            _subscriptions: subscriptions,
        }
    }

    fn duration(&self, cx: &App) -> Option<TimeMs> {
        self.state.read(cx).selected().map(|s| s.info().duration)
    }

    /// Every row as a segment, or `None` when the list is empty or any row is invalid.
    pub(super) fn segments(&self, cx: &App) -> Option<Vec<Segment>> {
        let duration = self.duration(cx)?;
        let segments: Result<Vec<_>, _> = self
            .rows
            .iter()
            .map(|row| {
                check_row(
                    &row.start.read(cx).value(),
                    &row.end.read(cx).value(),
                    duration,
                )
            })
            .collect();
        segments.ok().filter(|segments| !segments.is_empty())
    }

    fn add_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(duration) = self.duration(cx) else {
            return;
        };
        let start = self
            .rows
            .last()
            .map(|row| row.end.read(cx).value())
            .filter(|text| TimeMs::parse(text).is_ok())
            .map(|text| text.to_string())
            .unwrap_or_else(|| TimeMs(0).to_string());
        let end = duration.to_string();
        let id = self.next_id;
        self.next_id += 1;
        let start = cx.new(|cx| InputState::new(window, cx).default_value(start));
        let end = cx.new(|cx| InputState::new(window, cx).default_value(end));
        let subscriptions = [&start, &end]
            .into_iter()
            .map(|input| {
                cx.subscribe_in(input, window, |_, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                })
            })
            .collect();
        self.rows.push(Row {
            id,
            start,
            end,
            _subscriptions: subscriptions,
        });
        cx.notify();
    }

    fn remove_row(&mut self, id: u64, cx: &mut Context<Self>) {
        self.rows.retain(|row| row.id != id);
        cx.notify();
    }
}

fn selected_path(state: &Entity<AppState>, cx: &App) -> Option<PathBuf> {
    state.read(cx).selected().map(|s| s.path().to_owned())
}

/// A text element with a runtime id that tests and assistive tools can find.
pub(super) fn text_node(
    id: impl Into<SharedString>,
    text: impl Into<SharedString>,
) -> impl IntoElement + gpui_kit::Styled {
    let text = text.into();
    div()
        .id(id.into())
        .test_support()
        .aria_label(text.clone())
        .child(text)
}

impl Render for SegmentEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.settings.read(cx).language();
        let duration = self.duration(cx);
        let rows = self.rows.iter().map(|row| {
            let id = row.id;
            let error = duration.and_then(|duration| {
                check_row(
                    &row.start.read(cx).value(),
                    &row.end.read(cx).value(),
                    duration,
                )
                .err()
                .map(|error| error_text(error, duration, language))
            });
            let field = |key: &str, text: Text, input: &Entity<InputState>| {
                div()
                    .v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(div().text_sm().child(tr(language, text)))
                    .child(Input::new(input).id(SharedString::from(format!("segment-{id}-{key}"))))
            };
            div()
                .id(SharedString::from(format!("segment-{id}")))
                .v_flex()
                .gap_1()
                .child(
                    div()
                        .h_flex()
                        .items_end()
                        .gap_2()
                        .child(field("start", Text::SegmentStart, &row.start))
                        .child(field("end", Text::SegmentEnd, &row.end))
                        .child(
                            Button::new(SharedString::from(format!("segment-{id}-remove")))
                                .ghost()
                                .label(tr(language, Text::SegmentRemove))
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.remove_row(id, cx)),
                                ),
                        ),
                )
                .when_some(error, |this, text| {
                    this.child(
                        text_node(format!("segment-{id}-error"), text)
                            .text_sm()
                            .text_color(cx.theme().danger),
                    )
                })
        });
        div()
            .id("segment-editor")
            .test_support()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .id("segments-title")
                    .test_support()
                    .aria_label(tr(language, Text::SegmentsTitle))
                    .font_semibold()
                    .child(tr(language, Text::SegmentsTitle)),
            )
            .when(self.rows.is_empty(), |this| {
                this.child(
                    text_node("segments-empty", tr(language, Text::SegmentsEmptyHint))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
            })
            .children(rows)
            .child(
                Button::new("segment-add")
                    .label(tr(language, Text::SegmentAdd))
                    .disabled(duration.is_none())
                    .on_click(cx.listener(|this, _, window, cx| this.add_row(window, cx))),
            )
    }
}
