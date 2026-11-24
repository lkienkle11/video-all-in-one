//! User settings held in memory. Reading and writing the settings file is not done here.

use crate::i18n::Language;
use std::path::PathBuf;

/// Settings the app remembers between launches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    /// The chosen language, or `None` when the user has not chosen one yet.
    pub language: Option<Language>,
    /// The folder the user chose to find FFmpeg and FFprobe in, searched before `PATH`.
    pub tool_dir: Option<PathBuf>,
}

impl Settings {
    /// Returns true when the language picker must be shown.
    pub fn needs_language_choice(&self) -> bool {
        self.language.is_none()
    }
}
