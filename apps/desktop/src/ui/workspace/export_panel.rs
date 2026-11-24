//! Cut mode, export mode, output location and the Export command.
use super::segment_editor::{text_node, SegmentEditor};
use crate::{
    commands::export::{
        default_output_dir, default_stem, submit_export, ExportCommandError, ExportOptions,
    },
    state::AppState,
    ui::settings::{label, SettingsView},
};
use gpui_kit::TestSupportExt;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputState},
        radio::Radio,
        ActiveTheme, Disableable, StyledExt,
    },
    div,
    prelude::*,
    AppContext, Context, Entity, IntoElement, Render, SharedString, Subscription, Window,
};
use std::path::PathBuf;
use vaio_core::{
    domain::{
        export::{CutMode, ExportMode, PlanError},
        job::JobId,
        segment::SegmentError,
    },
    i18n::{tr, tr_fmt, Text},
};

enum Status {
    Idle,
    Submitting,
    Queued(JobId),
    Failed(ExportCommandError),
}

pub(super) struct ExportPanel {
    state: Entity<AppState>,
    settings: Entity<SettingsView>,
    editor: Entity<SegmentEditor>,
    folder: Entity<InputState>,
    stem: Entity<InputState>,
    cut: CutMode,
    mode: ExportMode,
    source: Option<PathBuf>,
    status: Status,
    _subscriptions: Vec<Subscription>,
}
impl ExportPanel {
    pub(super) fn new(
        state: Entity<AppState>,
        settings: Entity<SettingsView>,
        editor: Entity<SegmentEditor>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let folder = cx.new(|cx| InputState::new(window, cx));
        let stem = cx.new(|cx| InputState::new(window, cx));
        let subscriptions = vec![
            cx.observe_in(&state, window, |this, _, window, cx| {
                this.follow_source(window, cx)
            }),
            cx.observe(&editor, |_, _, cx| cx.notify()),
            cx.observe(&settings, |_, _, cx| cx.notify()),
        ];
        let mut panel = Self {
            state,
            settings,
            editor,
            folder,
            stem,
            cut: CutMode::Fast,
            mode: ExportMode::Separate,
            source: None,
            status: Status::Idle,
            _subscriptions: subscriptions,
        };
        panel.follow_source(window, cx);
        panel
    }

    /// A newly selected source gets its own default folder and name.
    fn follow_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.state.read(cx).selected().map(|s| s.path().to_owned());
        if selected == self.source {
            cx.notify();
            return;
        }
        self.source = selected.clone();
        self.status = Status::Idle;
        // A stat per candidate name; the planner re-checks before anything is written.
        let folder = selected
            .as_deref()
            .and_then(|path| default_output_dir(path, &|candidate| candidate.exists()))
            .map(|dir| dir.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stem = selected.as_deref().map(default_stem).unwrap_or_default();
        self.folder
            .update(cx, |input, cx| input.set_value(folder, window, cx));
        self.stem
            .update(cx, |input, cx| input.set_value(stem, window, cx));
        cx.notify();
    }

    fn export(&mut self, cx: &mut Context<Self>) {
        let Some(rows) = self.editor.read(cx).segments(cx) else {
            return;
        };
        let state = self.state.read(cx);
        let Some(source) = state.selected() else {
            self.status = Status::Failed(ExportCommandError::NoSource);
            cx.notify();
            return;
        };
        let Some(scheduler) = state.scheduler().cloned() else {
            self.status = Status::Failed(ExportCommandError::SchedulerUnavailable);
            cx.notify();
            return;
        };
        let (path, info) = (source.path().to_owned(), source.info().clone());
        let runtime = state.runtime().clone();
        let options = ExportOptions::new(
            self.cut,
            self.mode,
            PathBuf::from(self.folder.read(cx).value().trim()),
            self.stem.read(cx).value().trim().to_owned(),
        );
        self.status = Status::Submitting;
        cx.notify();
        let job = runtime
            .spawn_blocking(move || submit_export(&scheduler, &path, &info, &rows, &options));
        cx.spawn(async move |this, cx| {
            let result = job
                .await
                .map_err(|error| ExportCommandError::Submit(error.to_string()))
                .and_then(|result| result);
            let _ = this.update(cx, |this, cx| {
                this.status = match result {
                    Ok(id) => Status::Queued(id),
                    Err(error) => Status::Failed(error),
                };
                cx.notify();
            });
        })
        .detach();
    }
}

fn error_key(error: &ExportCommandError) -> Text {
    match error {
        ExportCommandError::NoSource => Text::ExportErrorNoSource,
        ExportCommandError::SchedulerUnavailable => Text::ExportErrorUnavailable,
        ExportCommandError::Segments(SegmentError::Empty) | ExportCommandError::Segments(_) => {
            Text::ExportErrorSegments
        }
        ExportCommandError::Plan(PlanError::InvalidStem) => Text::ExportErrorStem,
        ExportCommandError::Plan(PlanError::NotAbsolute(_)) => Text::ExportErrorFolder,
        ExportCommandError::Plan(PlanError::OutputEqualsSource) => Text::ExportErrorOverwrite,
        ExportCommandError::Plan(_) => Text::ExportErrorSegments,
        ExportCommandError::OutputNotDirectory(_) => Text::ExportErrorFolderIsFile,
        ExportCommandError::OutputCreate(_) => Text::ExportErrorFolderCreate,
        ExportCommandError::Payload(_) | ExportCommandError::Submit(_) => Text::ExportErrorSubmit,
    }
}

impl Render for ExportPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.settings.read(cx).language();
        let submitting = matches!(self.status, Status::Submitting);
        let can_export = self.editor.read(cx).segments(cx).is_some() && !submitting;
        let hint = match self.cut {
            CutMode::Fast => Text::ExportCutFastHint,
            CutMode::Precise => Text::ExportCutPreciseHint,
        };
        let field = |text: Text, input: &Entity<InputState>, id: &'static str| {
            div()
                .v_flex()
                .gap_1()
                .child(div().text_sm().child(tr(language, text)))
                .child(Input::new(input).id(id))
        };
        div()
            .id("export-panel")
            .test_support()
            .v_flex()
            .gap_2()
            .child(label("export-title", tr(language, Text::ExportTitle)).font_semibold())
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(
                        Radio::new("export-cut-fast")
                            .label(tr(language, Text::ExportCutFast))
                            .checked(self.cut == CutMode::Fast)
                            .on_change(cx.listener(|this, _, _, cx| {
                                this.cut = CutMode::Fast;
                                cx.notify();
                            })),
                    )
                    .child(
                        Radio::new("export-cut-precise")
                            .label(tr(language, Text::ExportCutPrecise))
                            .checked(self.cut == CutMode::Precise)
                            .on_change(cx.listener(|this, _, _, cx| {
                                this.cut = CutMode::Precise;
                                cx.notify();
                            })),
                    )
                    .child(
                        text_node("export-cut-hint", tr(language, hint))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(
                        Radio::new("export-mode-separate")
                            .label(tr(language, Text::ExportModeSeparate))
                            .checked(self.mode == ExportMode::Separate)
                            .on_change(cx.listener(|this, _, _, cx| {
                                this.mode = ExportMode::Separate;
                                cx.notify();
                            })),
                    )
                    .child(
                        Radio::new("export-mode-merged")
                            .label(tr(language, Text::ExportModeMerged))
                            .checked(self.mode == ExportMode::Merged)
                            .on_change(cx.listener(|this, _, _, cx| {
                                this.mode = ExportMode::Merged;
                                cx.notify();
                            })),
                    ),
            )
            .child(field(
                Text::ExportOutputFolder,
                &self.folder,
                "export-folder",
            ))
            .child(field(Text::ExportNameStem, &self.stem, "export-stem"))
            .child(
                Button::new("export-button")
                    .primary()
                    .label(tr(language, Text::ExportButton))
                    .disabled(!can_export)
                    .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
            )
            .child(match &self.status {
                Status::Idle => div().into_any_element(),
                Status::Submitting => {
                    text_node("export-status", tr(language, Text::ExportSubmitting))
                        .into_any_element()
                }
                Status::Queued(id) => text_node(
                    "export-status",
                    tr_fmt(language, Text::ExportQueued, &[&id.0.to_string()]),
                )
                .into_any_element(),
                Status::Failed(error) => text_node(
                    SharedString::from("export-error"),
                    tr(language, error_key(error)),
                )
                .text_color(cx.theme().danger)
                .into_any_element(),
            })
    }
}
