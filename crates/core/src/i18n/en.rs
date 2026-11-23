use super::Text;

/// English strings, one arm per text key.
pub fn text(text: Text) -> &'static str {
    match text {
        Text::AppTitle => "Video All-in-One",
        Text::ChooseLanguageTitle => "Choose your language",
        Text::ChooseLanguageBody => {
            "Select the language for the app. You can change it later in Settings."
        }
        Text::LanguageEnglish => "English",
        Text::LanguageVietnamese => "Tiếng Việt",
        Text::SettingsTitle => "Settings",
        Text::SettingsLanguage => "Language",
        Text::SettingsVersion => "Version {0}",
        Text::UpdateChecking => "Checking for updates…",
        Text::UpdateUpToDate => "You have the latest version",
        Text::UpdateAvailable => "Version {0} is available ({1} MB)",
        Text::UpdateDownload => "Download",
        Text::UpdateInstall => "Install",
        Text::UpdateDownloading => "Downloading {0} of {1} MB",
        Text::UpdateRetry => "Try again",
        Text::UpdateConfirmTitle => "Update now?",
        Text::UpdateConfirmBody => {
            "The app will restart to finish the update. Download size: {0} MB."
        }
        Text::UpdateLater => "Later",
        Text::UpdateNow => "Update now",
        Text::ErrorGeneric => "Something went wrong. Please try again.",
        Text::Close => "Close",
    }
}
