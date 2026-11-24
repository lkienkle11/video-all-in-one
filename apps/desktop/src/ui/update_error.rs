//! The modal that shows what the update helper left in `update-error.txt`.

use super::modal::{open_modal, ModalAction};
use gpui_kit::base::TestSupportExt;
use gpui_kit::prelude::*;
use gpui_kit::{div, App, IntoElement, RenderOnce, Window};
use std::path::Path;
use vaio_core::i18n::{tr, Language, Text};

/// Shows the file's text once and removes the file, so the next start does not repeat it.
pub fn show_update_error(data_dir: &Path, language: Language, window: &mut Window, cx: &mut App) {
    let path = data_dir.join(vaio_platform::update::UPDATE_ERROR_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let _ = std::fs::remove_file(&path);
    open_modal(
        window,
        cx,
        tr(language, Text::UpdateErrorFileTitle).into(),
        ErrorText(text),
        vec![ModalAction::new(
            "update-error-close",
            tr(language, Text::Close),
            |_, _| true,
        )],
    );
}

#[derive(Clone, IntoElement)]
struct ErrorText(String);

impl RenderOnce for ErrorText {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .id("update-error-text")
            .test_support()
            .aria_label(self.0.clone())
            .child(self.0)
    }
}
