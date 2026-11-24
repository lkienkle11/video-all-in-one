//! Bilingual UI strings: languages, text keys and the translation lookups.

mod en;
mod vi;

/// A language the UI can be shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Vietnamese,
}

impl Language {
    /// The stable short code stored in settings.
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Vietnamese => "vi",
        }
    }

    /// Parses a code produced by `code`. Matching is case-sensitive.
    pub fn from_code(code: &str) -> Option<Language> {
        match code {
            "en" => Some(Language::English),
            "vi" => Some(Language::Vietnamese),
            _ => None,
        }
    }
}

/// Every piece of UI text the app shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    AppTitle,
    ChooseLanguageTitle,
    ChooseLanguageBody,
    LanguageEnglish,
    LanguageVietnamese,
    SettingsTitle,
    SettingsLanguage,
    SettingsVersion,
    UpdateChecking,
    UpdateUpToDate,
    UpdateAvailable,
    UpdateDownload,
    UpdateInstall,
    UpdateDownloading,
    UpdateRetry,
    UpdateConfirmTitle,
    UpdateConfirmBody,
    UpdateLater,
    UpdateNow,
    ErrorGeneric,
    SettingsUpdates,
    UpdateCheck,
    UpdateReady,
    UpdateInstalled,
    UpdateFailed,
    UpdateInstallFailed,
    UpdateDownloaded,
    SettingsSaveFailed,
    Close,
    UpdateDownloadingUnknown,
    WorkspaceAddFile,
    WorkspaceSettings,
    SourcesTitle,
    SourcesEmpty,
    PreviewTitle,
    PreviewEmpty,
    InspectorTitle,
    InspectorEmpty,
    TimelineTitle,
    TimelineEmpty,
    QueueTitle,
    QueueEmpty,
    QueueCollapse,
    QueueExpand,
    SourceErrorTitle,
    SourceErrorBody,
    PreviewPlay,
    PreviewPause,
    PreviewSeek,
    PreviewLoading,
    PreviewFailed,
    JobQueued,
    JobRunning,
    JobSucceeded,
    JobFailed,
    JobCancelled,
    JobUnknown,
    InspectorContainer,
    InspectorVideo,
    InspectorAudio,
    SegmentsTitle,
    SegmentStart,
    SegmentEnd,
    SegmentAdd,
    SegmentRemove,
    SegmentsEmptyHint,
    SegmentErrorInvalid,
    SegmentErrorReversed,
    SegmentErrorBeyond,
    ExportTitle,
    ExportCutFast,
    ExportCutPrecise,
    ExportCutFastHint,
    ExportCutPreciseHint,
    ExportModeSeparate,
    ExportModeMerged,
    ExportOutputFolder,
    ExportNameStem,
    ExportButton,
    ExportQueued,
    ExportSubmitting,
    ExportErrorNoSource,
    ExportErrorUnavailable,
    ExportErrorSegments,
    ExportErrorStem,
    ExportErrorFolder,
    ExportErrorFolderIsFile,
    ExportErrorFolderCreate,
    ExportErrorOverwrite,
    ExportErrorSubmit,
    ProcessAllButton,
    ProcessAllQueued,
    ProcessAllSkipped,
    ProcessAllFailed,
    MissingToolTitle,
    MissingToolBody,
    MissingToolFolder,
    MissingToolCheck,
    MissingToolInvalid,
}

impl Text {
    /// Every text key, in the same order as the enum.
    pub const ALL: [Text; 99] = [
        Text::AppTitle,
        Text::ChooseLanguageTitle,
        Text::ChooseLanguageBody,
        Text::LanguageEnglish,
        Text::LanguageVietnamese,
        Text::SettingsTitle,
        Text::SettingsLanguage,
        Text::SettingsVersion,
        Text::UpdateChecking,
        Text::UpdateUpToDate,
        Text::UpdateAvailable,
        Text::UpdateDownload,
        Text::UpdateInstall,
        Text::UpdateDownloading,
        Text::UpdateRetry,
        Text::UpdateConfirmTitle,
        Text::UpdateConfirmBody,
        Text::UpdateLater,
        Text::UpdateNow,
        Text::ErrorGeneric,
        Text::SettingsUpdates,
        Text::UpdateCheck,
        Text::UpdateReady,
        Text::UpdateInstalled,
        Text::UpdateFailed,
        Text::UpdateInstallFailed,
        Text::UpdateDownloaded,
        Text::SettingsSaveFailed,
        Text::Close,
        Text::UpdateDownloadingUnknown,
        Text::WorkspaceAddFile,
        Text::WorkspaceSettings,
        Text::SourcesTitle,
        Text::SourcesEmpty,
        Text::PreviewTitle,
        Text::PreviewEmpty,
        Text::InspectorTitle,
        Text::InspectorEmpty,
        Text::TimelineTitle,
        Text::TimelineEmpty,
        Text::QueueTitle,
        Text::QueueEmpty,
        Text::QueueCollapse,
        Text::QueueExpand,
        Text::SourceErrorTitle,
        Text::SourceErrorBody,
        Text::PreviewPlay,
        Text::PreviewPause,
        Text::PreviewSeek,
        Text::PreviewLoading,
        Text::PreviewFailed,
        Text::JobQueued,
        Text::JobRunning,
        Text::JobSucceeded,
        Text::JobFailed,
        Text::JobCancelled,
        Text::JobUnknown,
        Text::InspectorContainer,
        Text::InspectorVideo,
        Text::InspectorAudio,
        Text::SegmentsTitle,
        Text::SegmentStart,
        Text::SegmentEnd,
        Text::SegmentAdd,
        Text::SegmentRemove,
        Text::SegmentsEmptyHint,
        Text::SegmentErrorInvalid,
        Text::SegmentErrorReversed,
        Text::SegmentErrorBeyond,
        Text::ExportTitle,
        Text::ExportCutFast,
        Text::ExportCutPrecise,
        Text::ExportCutFastHint,
        Text::ExportCutPreciseHint,
        Text::ExportModeSeparate,
        Text::ExportModeMerged,
        Text::ExportOutputFolder,
        Text::ExportNameStem,
        Text::ExportButton,
        Text::ExportQueued,
        Text::ExportSubmitting,
        Text::ExportErrorNoSource,
        Text::ExportErrorUnavailable,
        Text::ExportErrorSegments,
        Text::ExportErrorStem,
        Text::ExportErrorFolder,
        Text::ExportErrorFolderIsFile,
        Text::ExportErrorFolderCreate,
        Text::ExportErrorOverwrite,
        Text::ExportErrorSubmit,
        Text::ProcessAllButton,
        Text::ProcessAllQueued,
        Text::ProcessAllSkipped,
        Text::ProcessAllFailed,
        Text::MissingToolTitle,
        Text::MissingToolBody,
        Text::MissingToolFolder,
        Text::MissingToolCheck,
        Text::MissingToolInvalid,
    ];
}

/// Returns the raw text for a language, with any `{n}` placeholders unfilled.
pub fn tr(lang: Language, text: Text) -> &'static str {
    match lang {
        Language::English => en::text(text),
        Language::Vietnamese => vi::text(text),
    }
}

/// Returns the text with `{0}`, `{1}`, ... replaced by `args[0]`, `args[1]`, ...
/// A placeholder without an argument is kept as written; an extra argument is ignored.
/// Substitution is single-pass, so an argument is never expanded again.
pub fn tr_fmt(lang: Language, text: Text, args: &[&str]) -> String {
    let template = tr(lang, text);
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            break;
        };
        let key = &after[..end];
        let is_index = !key.is_empty() && key.bytes().all(|b| b.is_ascii_digit());
        match key
            .parse::<usize>()
            .ok()
            .filter(|_| is_index)
            .and_then(|i| args.get(i))
        {
            Some(arg) => out.push_str(arg),
            None => out.push_str(&rest[start..start + end + 2]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests;
