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
        Text::SettingsUpdates => "Updates",
        Text::UpdateCheck => "Check for updates",
        Text::UpdateReady => "Ready to install",
        Text::UpdateInstalled => "Update installed",
        Text::UpdateFailed => "Couldn’t update. Try again.",
        Text::UpdateInstallFailed => "Couldn’t install the update. Try again.",
        Text::UpdateDownloaded => "Downloaded {0} MB",
        Text::SettingsSaveFailed => "Couldn’t save your language. Try again.",
        Text::Close => "Close",
        Text::UpdateDownloadingUnknown => "Downloading {0} MB",
        Text::WorkspaceAddFile => "Add file…",
        Text::WorkspaceSettings => "Settings…",
        Text::SourcesTitle => "Sources",
        Text::SourcesEmpty => "No files yet",
        Text::PreviewTitle => "Preview",
        Text::PreviewEmpty => "Add a file to preview",
        Text::InspectorTitle => "Inspector",
        Text::InspectorEmpty => "Select a file to inspect",
        Text::TimelineTitle => "Timeline",
        Text::TimelineEmpty => "No segments yet",
        Text::QueueTitle => "Job queue",
        Text::QueueEmpty => "No jobs yet",
        Text::QueueCollapse => "Collapse queue",
        Text::QueueExpand => "Expand queue",
        Text::SourceErrorTitle => "Couldn’t open file",
        Text::SourceErrorBody => "Choose a readable video or audio file.",
        Text::PreviewPlay => "Play",
        Text::PreviewPause => "Pause",
        Text::PreviewSeek => "Seek",
        Text::PreviewLoading => "Loading preview…",
        Text::PreviewFailed => "Couldn’t play this file.",
        Text::JobQueued => "Queued",
        Text::JobRunning => "Running",
        Text::JobSucceeded => "Completed",
        Text::JobFailed => "Failed",
        Text::JobCancelled => "Cancelled",
        Text::JobUnknown => "Waiting for status",
        Text::InspectorContainer => "Container: {0}",
        Text::InspectorVideo => "Video: {0} · {1} × {2}",
        Text::InspectorAudio => "Audio: {0} · {1} channels",
        Text::SegmentsTitle => "Segments",
        Text::SegmentStart => "Start",
        Text::SegmentEnd => "End",
        Text::SegmentAdd => "Add segment",
        Text::SegmentRemove => "Remove",
        Text::SegmentsEmptyHint => "Add at least one segment to export.",
        Text::SegmentErrorInvalid => "Enter a time like 90, 01:30 or 00:01:30.500.",
        Text::SegmentErrorReversed => "The end must be after the start.",
        Text::SegmentErrorBeyond => "The end is after the end of the video ({0}).",
        Text::ExportTitle => "Export",
        Text::ExportCutFast => "Fast",
        Text::ExportCutPrecise => "Precise",
        Text::ExportCutFastHint => {
            "Fast copies the video without re-encoding, so cuts snap to the nearest keyframe."
        }
        Text::ExportCutPreciseHint => {
            "Precise re-encodes the video to cut exactly at the times you set. It takes longer."
        }
        Text::ExportModeSeparate => "Separate files",
        Text::ExportModeMerged => "One merged file",
        Text::ExportOutputFolder => "Output folder",
        Text::ExportNameStem => "File name",
        Text::ExportButton => "Export",
        Text::ExportQueued => "Export queued as job #{0}.",
        Text::ExportSubmitting => "Adding to the queue…",
        Text::ExportErrorNoSource => "Add and select a video first.",
        Text::ExportErrorUnavailable => "The export queue is not available.",
        Text::ExportErrorSegments => "Fix the segments before exporting.",
        Text::ExportErrorStem => {
            "The file name is empty, too long or has a character that is not allowed."
        }
        Text::ExportErrorFolder => "Enter the output folder as an absolute path.",
        Text::ExportErrorFolderIsFile => "The output folder is an existing file.",
        Text::ExportErrorFolderCreate => "Couldn’t create the output folder.",
        Text::ExportErrorOverwrite => "The output would overwrite the source video.",
        Text::ExportErrorSubmit => "Couldn’t add the export to the queue. Try again.",
        Text::ProcessAllButton => "Process all",
        Text::ProcessAllQueued => "{0} exports queued.",
        Text::ProcessAllSkipped => "{0} without segments skipped.",
        Text::ProcessAllFailed => "Nothing was queued. “{0}” can’t be exported: {1}",
    }
}
