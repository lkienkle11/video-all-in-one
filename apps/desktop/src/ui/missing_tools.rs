//! The modal that names a missing FFmpeg or FFprobe and lets the user choose their folder.

use super::{
    modal::{open_dynamic_modal, ModalAction},
    settings::{label, SettingsView},
};
use crate::state::AppState;
use gpui_kit::{
    component::input::{Input, InputState},
    div,
    prelude::*,
    App, AppContext, Context, Entity, IntoElement, Render, Window,
};
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};
use vaio_core::{
    domain::tool::Tool,
    i18n::{tr, tr_fmt, Text},
};
use vaio_media::ffmpeg::FfmpegPaths;
use vaio_platform::binary_resolver::{resolve_tool_paths_in, ResolveError};

/// Resolves FFmpeg and FFprobe from `dir` first and then from `path_var`.
/// The error names the first tool that was not found.
pub fn check_tools(dir: Option<&Path>, path_var: Option<&OsStr>) -> Result<FfmpegPaths, Tool> {
    resolve_tool_paths_in(dir, path_var)
        .map(|found| FfmpegPaths {
            ffmpeg: found.ffmpeg,
            ffprobe: found.ffprobe,
        })
        .map_err(|error| match error {
            ResolveError::Missing { tool, .. } => tool,
            // A folder that is not absolute is never searched; report the first tool.
            ResolveError::ConfiguredDirNotAbsolute(_) => Tool::Ffmpeg,
        })
}

/// The shell command that installs `tool` on `os` (a `std::env::consts::OS` value).
/// The platform resolver words the same command inside an English sentence only.
pub fn install_command(tool: Tool, os: &str) -> &'static str {
    match (os, tool) {
        ("macos", Tool::YtDlp) => "brew install yt-dlp",
        ("macos", _) => "brew install ffmpeg",
        ("windows", Tool::YtDlp) => "winget install yt-dlp",
        ("windows", _) => "winget install ffmpeg",
        (_, Tool::YtDlp) => "pipx install yt-dlp",
        (_, _) => "sudo apt install ffmpeg",
    }
}

fn display_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Ffmpeg => "FFmpeg",
        Tool::Ffprobe => "FFprobe",
        Tool::YtDlp => "yt-dlp",
    }
}

/// Makes sure FFmpeg and FFprobe can be found. When one cannot, opens the modal and returns false.
/// Without a tool search on the state the check is off and this returns true.
pub fn ensure_tools(
    state: &Entity<AppState>,
    settings: &Entity<SettingsView>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(search) = state.read(cx).tool_search().cloned() else {
        return true;
    };
    let dir = settings.read(cx).tool_dir().map(Path::to_owned);
    match check_tools(dir.as_deref(), search.path_var()) {
        Ok(paths) => {
            state.read(cx).set_tools(paths);
            true
        }
        Err(tool) => {
            open(
                state,
                settings,
                tool,
                search.path_var().map(OsString::from),
                window,
                cx,
            );
            false
        }
    }
}

struct Body {
    settings: Entity<SettingsView>,
    folder: Entity<InputState>,
    tool: Tool,
    invalid: bool,
}
impl Render for Body {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.settings.read(cx).language();
        let name = display_name(self.tool);
        let command = install_command(self.tool, std::env::consts::OS);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(label(
                "missing-tool-body",
                tr_fmt(language, Text::MissingToolBody, &[name, command]),
            ))
            .child(div().text_sm().child(tr(language, Text::MissingToolFolder)))
            .child(Input::new(&self.folder).id("missing-tool-folder"))
            .when(self.invalid, |this| {
                this.child(label(
                    "missing-tool-invalid",
                    tr(language, Text::MissingToolInvalid),
                ))
            })
    }
}

fn open(
    state: &Entity<AppState>,
    settings: &Entity<SettingsView>,
    tool: Tool,
    path_var: Option<OsString>,
    window: &mut Window,
    cx: &mut App,
) {
    let current = settings.read(cx).tool_dir().map(Path::to_owned);
    let folder = cx.new(|cx| {
        let mut input = InputState::new(window, cx);
        if let Some(dir) = &current {
            input.set_value(dir.to_string_lossy().into_owned(), window, cx);
        }
        input
    });
    let body = cx.new(|_| Body {
        settings: settings.clone(),
        folder: folder.clone(),
        tool,
        invalid: false,
    });
    let (state, settings, kept) = (state.clone(), settings.clone(), body.clone());
    open_dynamic_modal(window, cx, body, move |cx| {
        let language = settings.read(cx).language();
        let (state, settings, body, folder, path_var) = (
            state.clone(),
            settings.clone(),
            kept.clone(),
            folder.clone(),
            path_var.clone(),
        );
        let check = ModalAction::new(
            "missing-tools-check",
            tr(language, Text::MissingToolCheck),
            move |_, cx| {
                let text = folder.read(cx).value().trim().to_owned();
                let dir = (!text.is_empty()).then(|| PathBuf::from(text));
                match check_tools(dir.as_deref(), path_var.as_deref()) {
                    Ok(paths) => {
                        state.read(cx).set_tools(paths);
                        if let Some(dir) = dir {
                            settings.update(cx, |view, cx| view.set_tool_dir(dir, cx));
                        }
                        true
                    }
                    Err(_) => {
                        body.update(cx, |body, cx| {
                            body.invalid = true;
                            cx.notify();
                        });
                        false
                    }
                }
            },
        );
        (
            tr_fmt(language, Text::MissingToolTitle, &[display_name(tool)]).into(),
            vec![
                check,
                ModalAction::new("missing-tools-close", tr(language, Text::Close), |_, _| {
                    true
                }),
            ],
        )
    });
}
