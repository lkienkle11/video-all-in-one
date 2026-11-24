//! The local-media workspace shell.
use crate::ui::{
    modal::{open_dynamic_modal, ModalAction},
    settings::label,
};
use crate::{state::AppState, ui::settings::SettingsView};
use gpui_kit::{
    component::{
        resizable::{h_resizable, resizable_panel, v_resizable},
        ActiveTheme,
    },
    div,
    prelude::*,
    AppContext, Context, Entity, IntoElement, PathPromptOptions, Render, Subscription, Task,
    Window,
};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use vaio_core::{
    domain::media::MediaInfo,
    i18n::{tr, Language, Text},
};
use vaio_media::analysis::analyze_source;

mod inspector;
mod job_queue_panel;
mod preview;
mod source_list;
mod timeline_placeholder;
mod toolbar;

pub struct WorkspaceView {
    state: Entity<AppState>,
    settings: Entity<SettingsView>,
    preview: Entity<preview::PreviewView>,
    queue_collapsed: bool,
    cancel: CancellationToken,
    _subscriptions: Vec<Subscription>,
}
impl WorkspaceView {
    pub fn new(
        state: Entity<AppState>,
        settings: Entity<SettingsView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::with_audio(state, settings, true, window, cx)
    }
    /// Tests use null audio; the application constructor enables real audio.
    pub fn with_audio(
        state: Entity<AppState>,
        settings: Entity<SettingsView>,
        audio: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let runtime = state.read(cx).runtime().clone();
        let preview =
            cx.new(|cx| preview::PreviewView::new(runtime, audio, settings.clone(), window, cx));
        let subscriptions = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.observe(&settings, |_, _, cx| cx.notify()),
        ];
        state.update(cx, |state, cx| state.observe_jobs(cx));
        Self {
            state,
            settings,
            preview,
            queue_collapsed: false,
            cancel: CancellationToken::new(),
            _subscriptions: subscriptions,
        }
    }
    fn language(&self, cx: &gpui_kit::App) -> Language {
        self.settings.read(cx).language()
    }

    /// Starts analysis on Tokio and returns after the source or error modal has reached the UI.
    pub fn add_source_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<MediaInfo, String>> {
        let state = self.state.read(cx);
        let tools = state.tools();
        let cancel = self.cancel.child_token();
        let source = path.clone();
        let analysis = state.runtime().spawn(async move {
            let tools = tools.ok_or_else(|| "FFprobe is unavailable".to_owned())?;
            analyze_source(&source, &tools, cancel)
                .await
                .map_err(|error| error.to_string())
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = analysis
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| match &result {
                Ok(info) => {
                    view.state.update(cx, |state, cx| {
                        state.add_source(path.clone(), info.clone(), cx)
                    });
                    view.preview
                        .update(cx, |preview, cx| preview.load(path, window, cx));
                }
                Err(_) => view.show_source_error(window, cx),
            });
            result
        })
    }
    fn select_source(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.state
            .update(cx, |state, cx| state.select(path.clone(), cx));
        self.preview
            .update(cx, |preview, cx| preview.load(path, window, cx));
    }
    fn show_source_error(&self, window: &mut Window, cx: &mut Context<Self>) {
        let body = cx.new(|_| SourceError(self.settings.clone()));
        let settings = self.settings.clone();
        open_dynamic_modal(window, cx, body, move |cx| {
            let language = settings.read(cx).language();
            (
                tr(language, Text::SourceErrorTitle).into(),
                vec![ModalAction::new(
                    "source-error-close",
                    tr(language, Text::Close),
                    |_, _| true,
                )],
            )
        });
    }
    fn open_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = self.settings.clone();
        open_dynamic_modal(window, cx, self.settings.clone(), move |cx| {
            let language = settings.read(cx).language();
            (
                tr(language, Text::SettingsTitle).into(),
                vec![ModalAction::new(
                    "settings-close",
                    tr(language, Text::Close),
                    |_, _| true,
                )],
            )
        });
    }
    fn prompt_for_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr(self.language(cx), Text::WorkspaceAddFile).into()),
        });
        cx.spawn_in(window, async move |view, cx| match prompt.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.into_iter().next() {
                    let task = view.update_in(cx, |view, window, cx| {
                        view.add_source_path(path, window, cx)
                    });
                    if let Ok(task) = task {
                        let _ = task.await;
                    }
                }
            }
            Ok(Ok(None)) => {}
            _ => {
                let _ = view.update_in(cx, |view, window, cx| view.show_source_error(window, cx));
            }
        })
        .detach();
    }
}
impl Drop for WorkspaceView {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rem = window.rem_size();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(toolbar::render(self, cx))
            .child(
                div().flex_1().min_h_0().min_w_0().child(
                    v_resizable("workspace-vertical")
                        .child(
                            resizable_panel().size_range(rem * 16. ..rem * 1000.).child(
                                h_resizable("workspace-horizontal")
                                    .child(
                                        resizable_panel()
                                            .size(rem * 16.)
                                            .size_range(rem * 10. ..rem * 28.)
                                            .child(source_list::render(self, cx)),
                                    )
                                    .child(
                                        resizable_panel()
                                            .size_range(rem * 16. ..rem * 1000.)
                                            .child(self.preview.clone()),
                                    )
                                    .child(
                                        resizable_panel()
                                            .size(rem * 18.)
                                            .size_range(rem * 12. ..rem * 28.)
                                            .child(inspector::render(self, cx)),
                                    ),
                            ),
                        )
                        .child(
                            resizable_panel()
                                .size(rem * 7.)
                                .size_range(rem * 5. ..rem * 14.)
                                .child(timeline_placeholder::render(self.language(cx), cx)),
                        )
                        .when(!self.queue_collapsed, |this| {
                            this.child(
                                resizable_panel()
                                    .size(rem * 10.)
                                    .size_range(rem * 6. ..rem * 24.)
                                    .child(job_queue_panel::render(self, cx)),
                            )
                        }),
                ),
            )
            .when(self.queue_collapsed, |this| {
                this.child(div().h_12().child(job_queue_panel::render(self, cx)))
            })
    }
}

struct SourceError(Entity<SettingsView>);
impl Render for SourceError {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        label(
            "source-error-body",
            tr(self.0.read(cx).language(), Text::SourceErrorBody),
        )
    }
}
