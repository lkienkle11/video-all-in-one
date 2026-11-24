use super::*;
use crate::settings::Settings;

#[test]
fn every_text_has_non_empty_english_and_vietnamese() {
    for text in Text::ALL {
        assert!(
            !tr(Language::English, text).is_empty(),
            "{text:?} has no English text"
        );
        assert!(
            !tr(Language::Vietnamese, text).is_empty(),
            "{text:?} has no Vietnamese text"
        );
    }
}

#[test]
fn vietnamese_differs_from_english_for_every_text_except_app_title() {
    for text in Text::ALL {
        // The app name and the language names are the same word in both languages.
        if matches!(
            text,
            Text::AppTitle | Text::LanguageEnglish | Text::LanguageVietnamese
        ) {
            continue;
        }
        assert_ne!(
            tr(Language::English, text),
            tr(Language::Vietnamese, text),
            "{text:?} is not translated"
        );
    }
}

#[test]
fn placeholder_count_matches_between_languages() {
    for text in Text::ALL {
        let english = tr(Language::English, text).matches('{').count();
        let vietnamese = tr(Language::Vietnamese, text).matches('{').count();
        assert_eq!(
            english, vietnamese,
            "{text:?} has a different placeholder count"
        );
    }
}

#[test]
fn tr_fmt_fills_indexed_placeholders_in_their_own_positions() {
    let text = tr_fmt(Language::English, Text::UpdateDownloading, &["AAA", "BBB"]);
    assert!(text.contains("AAA"), "first argument missing: {text}");
    assert!(text.contains("BBB"), "second argument missing: {text}");
    assert!(
        text.find("AAA") < text.find("BBB"),
        "arguments out of order: {text}"
    );
    assert!(!text.contains("{0}") && !text.contains("{1}"));
}

#[test]
fn tr_fmt_leaves_placeholder_without_argument_as_is() {
    let text = tr_fmt(Language::English, Text::UpdateAvailable, &["1.2.0"]);
    assert!(text.contains("1.2.0"));
    assert!(
        text.contains("{1}"),
        "missing placeholder was dropped: {text}"
    );
}

#[test]
fn tr_fmt_ignores_extra_argument() {
    let with_extra = tr_fmt(
        Language::English,
        Text::UpdateAvailable,
        &["1.2.0", "45.5", "EXTRA"],
    );
    let without = tr_fmt(Language::English, Text::UpdateAvailable, &["1.2.0", "45.5"]);
    assert_eq!(with_extra, without);
    assert!(!with_extra.contains("EXTRA"));
}

#[test]
fn tr_fmt_does_not_expand_placeholders_inside_arguments() {
    let text = tr_fmt(Language::English, Text::UpdateAvailable, &["{1}", "45.5"]);
    assert_eq!(
        text.matches("45.5").count(),
        1,
        "argument was re-expanded: {text}"
    );
    assert_eq!(
        text.matches("{1}").count(),
        1,
        "argument was re-expanded: {text}"
    );
}

#[test]
fn needs_language_choice_when_unset() {
    assert!(Settings::default().needs_language_choice());
    let chosen = Settings {
        language: Some(Language::Vietnamese),
    };
    assert!(!chosen.needs_language_choice());
}

/// Position of each text in the enum, numbered from 0. Exhaustive on purpose:
/// a new variant fails to compile until it is numbered here.
fn index(t: Text) -> usize {
    match t {
        Text::AppTitle => 0,
        Text::ChooseLanguageTitle => 1,
        Text::ChooseLanguageBody => 2,
        Text::LanguageEnglish => 3,
        Text::LanguageVietnamese => 4,
        Text::SettingsTitle => 5,
        Text::SettingsLanguage => 6,
        Text::SettingsVersion => 7,
        Text::UpdateChecking => 8,
        Text::UpdateUpToDate => 9,
        Text::UpdateAvailable => 10,
        Text::UpdateDownload => 11,
        Text::UpdateInstall => 12,
        Text::UpdateDownloading => 13,
        Text::UpdateRetry => 14,
        Text::UpdateConfirmTitle => 15,
        Text::UpdateConfirmBody => 16,
        Text::UpdateLater => 17,
        Text::UpdateNow => 18,
        Text::ErrorGeneric => 19,
        Text::SettingsUpdates => 20,
        Text::UpdateCheck => 21,
        Text::UpdateReady => 22,
        Text::UpdateInstalled => 23,
        Text::UpdateFailed => 24,
        Text::UpdateInstallFailed => 25,
        Text::UpdateDownloaded => 26,
        Text::SettingsSaveFailed => 27,
        Text::Close => 28,
        Text::UpdateDownloadingUnknown => 29,
        Text::WorkspaceAddFile => 30,
        Text::WorkspaceSettings => 31,
        Text::SourcesTitle => 32,
        Text::SourcesEmpty => 33,
        Text::PreviewTitle => 34,
        Text::PreviewEmpty => 35,
        Text::InspectorTitle => 36,
        Text::InspectorEmpty => 37,
        Text::TimelineTitle => 38,
        Text::TimelineEmpty => 39,
        Text::QueueTitle => 40,
        Text::QueueEmpty => 41,
        Text::QueueCollapse => 42,
        Text::QueueExpand => 43,
        Text::SourceErrorTitle => 44,
        Text::SourceErrorBody => 45,
        Text::PreviewPlay => 46,
        Text::PreviewPause => 47,
        Text::PreviewSeek => 48,
        Text::PreviewLoading => 49,
        Text::PreviewFailed => 50,
        Text::JobQueued => 51,
        Text::JobRunning => 52,
        Text::JobSucceeded => 53,
        Text::JobFailed => 54,
        Text::JobCancelled => 55,
        Text::JobUnknown => 56,
        Text::InspectorContainer => 57,
        Text::InspectorVideo => 58,
        Text::InspectorAudio => 59,
        Text::SegmentsTitle => 60,
        Text::SegmentStart => 61,
        Text::SegmentEnd => 62,
        Text::SegmentAdd => 63,
        Text::SegmentRemove => 64,
        Text::SegmentsEmptyHint => 65,
        Text::SegmentErrorInvalid => 66,
        Text::SegmentErrorReversed => 67,
        Text::SegmentErrorBeyond => 68,
        Text::ExportTitle => 69,
        Text::ExportCutFast => 70,
        Text::ExportCutPrecise => 71,
        Text::ExportCutFastHint => 72,
        Text::ExportCutPreciseHint => 73,
        Text::ExportModeSeparate => 74,
        Text::ExportModeMerged => 75,
        Text::ExportOutputFolder => 76,
        Text::ExportNameStem => 77,
        Text::ExportButton => 78,
        Text::ExportQueued => 79,
        Text::ExportSubmitting => 80,
        Text::ExportErrorNoSource => 81,
        Text::ExportErrorUnavailable => 82,
        Text::ExportErrorSegments => 83,
        Text::ExportErrorStem => 84,
        Text::ExportErrorFolder => 85,
        Text::ExportErrorFolderIsFile => 86,
        Text::ExportErrorFolderCreate => 87,
        Text::ExportErrorOverwrite => 88,
        Text::ExportErrorSubmit => 89,
        Text::ProcessAllButton => 90,
        Text::ProcessAllQueued => 91,
        Text::ProcessAllSkipped => 92,
        Text::ProcessAllFailed => 93,
    }
}

#[test]
fn text_all_lists_every_variant_in_enum_order() {
    for (position, text) in Text::ALL.iter().enumerate() {
        assert_eq!(
            index(*text),
            position,
            "Text::ALL[{position}] is {text:?}, which is out of enum order"
        );
    }
    // The last variant has the highest index.
    let highest = index(Text::ProcessAllFailed);
    assert_eq!(
        Text::ALL.len(),
        highest + 1,
        "Text::ALL does not list every variant"
    );
}

/// The numbers of the well-formed `{N}` placeholders in `s`, in order of appearance.
fn placeholder_numbers(s: &str) -> Vec<usize> {
    let mut numbers = Vec::new();
    let mut rest = s;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            break;
        };
        let key = &after[..end];
        if !key.is_empty() && key.bytes().all(|b| b.is_ascii_digit()) {
            if let Ok(number) = key.parse::<usize>() {
                numbers.push(number);
            }
        }
        rest = &after[end + 1..];
    }
    numbers
}

#[test]
fn placeholders_appear_in_the_same_order_in_both_languages() {
    for text in Text::ALL {
        assert_eq!(
            placeholder_numbers(tr(Language::English, text)),
            placeholder_numbers(tr(Language::Vietnamese, text)),
            "{text:?} has its placeholders in a different order"
        );
    }
}

#[test]
fn language_roundtrips_through_code() {
    assert_eq!(Language::English.code(), "en");
    assert_eq!(Language::Vietnamese.code(), "vi");
    assert_eq!(Language::from_code("en"), Some(Language::English));
    assert_eq!(Language::from_code("vi"), Some(Language::Vietnamese));
    assert_eq!(Language::from_code("EN"), None);
    assert_eq!(Language::from_code("fr"), None);
    assert_eq!(Language::from_code(""), None);
    for language in [Language::English, Language::Vietnamese] {
        assert_eq!(Language::from_code(language.code()), Some(language));
    }
}
