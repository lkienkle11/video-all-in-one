use anyhow::{bail, Result};

/// Title of the heading that introduces the Vietnamese notes in CHANGELOG.md (the language's own name).
/// This is the only Vietnamese text in the program source.
pub(crate) const VIETNAMESE_TITLE: &str = "Tiếng Việt";
const ENGLISH_TITLE: &str = "English";
/// Words that mark unfinished notes; compared case-insensitively.
const PLACEHOLDERS: [&str; 6] = ["tbd", "todo", "lorem", "placeholder", "xxx", "coming soon"];

/// The English and Vietnamese release notes of one version, Markdown kept as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notes {
    pub english: String,
    pub vietnamese: String,
}

/// Reads the notes of `version` from a CHANGELOG.md text.
pub fn read_notes(changelog: &str, version: &str) -> Result<Notes> {
    let lines: Vec<&str> = changelog.lines().collect();
    let section = version_section(&lines, version)?;
    let english = language_notes(&section, ENGLISH_TITLE, version)?;
    let vietnamese = language_notes(&section, VIETNAMESE_TITLE, version)?;
    Ok(Notes {
        english,
        vietnamese,
    })
}

/// The lines under the `## <version>` heading, up to the next `##` (or `#`) heading.
/// Headings inside fenced code blocks do not count.
fn version_section<'a>(lines: &[&'a str], version: &str) -> Result<Vec<&'a str>> {
    let outside = outside_code_blocks(lines);
    let Some(start) = lines
        .iter()
        .enumerate()
        .position(|(index, line)| outside[index] && heading_version(line) == Some(version))
    else {
        bail!("CHANGELOG.md has no section for version {version}");
    };
    let body = &lines[start + 1..];
    let body_outside = outside_code_blocks(body);
    let end = body
        .iter()
        .zip(&body_outside)
        .position(|(line, &outside)| outside && heading_level(line).is_some_and(|level| level <= 2))
        .unwrap_or(body.len());
    Ok(body[..end].to_vec())
}

/// The lines under `### <title>` inside a version section, up to the next `###` or `##`.
/// Headings inside fenced code blocks do not count.
fn language_notes(section: &[&str], title: &str, version: &str) -> Result<String> {
    let outside = outside_code_blocks(section);
    let Some(start) = section
        .iter()
        .enumerate()
        .position(|(index, line)| outside[index] && heading_title(line, 3) == Some(title))
    else {
        bail!("version {version} has no `### {title}` heading");
    };
    let body = &section[start + 1..];
    let body_outside = outside_code_blocks(body);
    let end = body
        .iter()
        .zip(&body_outside)
        .position(|(line, &outside)| outside && heading_level(line).is_some_and(|level| level <= 3))
        .unwrap_or(body.len());
    let text = body[..end].join("\n").trim().to_string();
    if text.is_empty() {
        bail!("version {version} has empty {title} notes");
    }
    if let Some(word) = placeholder_in(&text) {
        bail!("version {version} {title} notes contain placeholder text `{word}`");
    }
    Ok(text)
}

/// For each line, whether it lies outside a fenced code block. A fence line (```` ``` ```` or
/// `~~~`) and every line up to the closing fence are inside the block.
fn outside_code_blocks(lines: &[&str]) -> Vec<bool> {
    let mut fence: Option<&str> = None;
    lines
        .iter()
        .map(|line| {
            let trimmed = line.trim_start();
            match fence {
                Some(marker) => {
                    if trimmed.starts_with(marker) {
                        fence = None;
                    }
                    false
                }
                None => {
                    fence = ["```", "~~~"]
                        .into_iter()
                        .find(|marker| trimmed.starts_with(marker));
                    fence.is_none()
                }
            }
        })
        .collect()
}

/// The first placeholder that occurs in `text` as whole words, case-insensitively.
fn placeholder_in(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    PLACEHOLDERS.iter().copied().find(|placeholder| {
        let parts: Vec<&str> = placeholder.split(' ').collect();
        words
            .windows(parts.len())
            .any(|window| window == parts.as_slice())
    })
}

/// The version named by a `## X.Y.Z`, `## X.Y.Z - <date>`, `## [X.Y.Z]` or
/// `## [X.Y.Z] - <date>` heading.
fn heading_version(line: &str) -> Option<&str> {
    let title = heading_title(line, 2)?;
    match title.strip_prefix('[') {
        Some(rest) => rest.split_once(']').map(|(version, _)| version),
        None => title.split_whitespace().next(),
    }
}

/// The trimmed text of a heading that has exactly `level` hashes, or `None`.
fn heading_title(line: &str, level: usize) -> Option<&str> {
    if heading_level(line) != Some(level) {
        return None;
    }
    line[level..].strip_prefix(' ').map(str::trim)
}

/// The level of a Markdown ATX heading (`#` to `######`), or `None` for other lines.
fn heading_level(line: &str) -> Option<usize> {
    let hashes = line.len() - line.trim_start_matches('#').len();
    (hashes > 0 && line[hashes..].starts_with(' ')).then_some(hashes)
}

#[cfg(test)]
mod tests {
    use super::{read_notes, VIETNAMESE_TITLE};

    fn section(version: &str, english: &str, vietnamese: &str) -> String {
        format!(
            "# Changelog\n\n## {version}\n\n### English\n\n{english}\n\n### {VIETNAMESE_TITLE}\n\n{vietnamese}\n"
        )
    }

    fn error_text(changelog: &str, version: &str) -> String {
        format!("{:#}", read_notes(changelog, version).unwrap_err())
    }

    #[test]
    fn reads_both_languages_of_a_version_section() {
        let text = section("0.1.0", "- Added export", "- Vietnamese export");
        let notes = read_notes(&text, "0.1.0").unwrap();
        assert_eq!(notes.english, "- Added export");
        assert_eq!(notes.vietnamese, "- Vietnamese export");
    }

    #[test]
    fn accepts_bracketed_version_headings_with_a_date() {
        let bracketed = format!(
            "## [0.1.0] - 2026-11-23\n\n### English\n\nEn\n\n### {VIETNAMESE_TITLE}\n\nVi\n"
        );
        let notes = read_notes(&bracketed, "0.1.0").unwrap();
        assert_eq!(notes.english, "En");
        assert_eq!(notes.vietnamese, "Vi");

        let plain_with_date = format!(
            "## 0.2.0 - 2026-11-23\n\n### English\n\nEn2\n\n### {VIETNAMESE_TITLE}\n\nVi2\n"
        );
        assert_eq!(
            read_notes(&plain_with_date, "0.2.0").unwrap().english,
            "En2"
        );
    }

    #[test]
    fn stops_at_the_next_version_section() {
        let text = format!(
            "## 0.2.0\n\n### English\n\nNew\n\n### {VIETNAMESE_TITLE}\n\nNew vi\n\n## 0.1.0\n\n### English\n\nOld\n\n### {VIETNAMESE_TITLE}\n\nOld vi\n"
        );
        let notes = read_notes(&text, "0.2.0").unwrap();
        assert_eq!(notes.english, "New");
        assert_eq!(notes.vietnamese, "New vi");

        let no_english_of_its_own = format!(
            "## 0.2.0\n\n### {VIETNAMESE_TITLE}\n\nOnly vi\n\n## 0.1.0\n\n### English\n\nOld\n"
        );
        let message = error_text(&no_english_of_its_own, "0.2.0");
        assert!(message.contains("English"), "{message}");
    }

    #[test]
    fn fails_when_the_vietnamese_section_is_missing() {
        let text = "## 0.1.0\n\n### English\n\nEn\n";
        let message = error_text(text, "0.1.0");
        assert!(message.contains("0.1.0"), "{message}");
        assert!(message.contains(VIETNAMESE_TITLE), "{message}");
    }

    #[test]
    fn fails_when_the_english_section_is_missing() {
        let text = format!("## 0.1.0\n\n### {VIETNAMESE_TITLE}\n\nVi\n");
        let message = error_text(&text, "0.1.0");
        assert!(message.contains("0.1.0"), "{message}");
        assert!(message.contains("English"), "{message}");
    }

    #[test]
    fn fails_when_the_version_section_is_missing() {
        let text = section("0.1.0", "En", "Vi");
        let message = error_text(&text, "0.3.0");
        assert!(message.contains("0.3.0"), "{message}");
    }

    #[test]
    fn fails_on_placeholder_text() {
        let text = section("0.1.0", "TODO write the notes", "Vi");
        let message = error_text(&text, "0.1.0");
        assert!(message.contains("0.1.0"), "{message}");
        assert!(message.contains("English"), "{message}");
    }

    #[test]
    fn fails_on_empty_notes() {
        let text = section("0.1.0", "", "Vi");
        let message = error_text(&text, "0.1.0");
        assert!(message.contains("0.1.0"), "{message}");
        assert!(message.contains("English"), "{message}");
    }

    #[test]
    fn does_not_mix_up_0_1_1_and_0_1_10() {
        let text = section("0.1.10", "Ten", "Ten vi");
        let message = error_text(&text, "0.1.1");
        assert!(message.contains("0.1.1"), "{message}");
    }

    #[test]
    fn heading_inside_a_code_block_does_not_end_the_section() {
        let text = format!(
            "## 0.1.0\n\n### English\n\nBefore\n\n```md\n## Not a heading\n### Also not\n```\n\nAfter\n\n### {VIETNAMESE_TITLE}\n\nVi\n"
        );
        let notes = read_notes(&text, "0.1.0").unwrap();
        assert!(
            notes.english.contains("## Not a heading"),
            "{}",
            notes.english
        );
        assert!(notes.english.ends_with("After"), "{}", notes.english);
        assert_eq!(notes.vietnamese, "Vi");
    }

    #[test]
    fn version_heading_inside_a_code_block_is_ignored() {
        let text = format!(
            "```text\n## 0.1.0\n```\n\n## 0.2.0\n\n### English\n\nEn\n\n### {VIETNAMESE_TITLE}\n\nVi\n"
        );
        let message = error_text(&text, "0.1.0");
        assert!(message.contains("0.1.0"), "{message}");
        assert!(message.contains("no section"), "{message}");
        assert_eq!(read_notes(&text, "0.2.0").unwrap().english, "En");
    }

    #[test]
    fn todo_inside_a_longer_word_is_not_a_placeholder() {
        for english in ["Added autodocs export", "New xxxl size option"] {
            let text = section("0.1.0", english, "Vi");
            assert!(read_notes(&text, "0.1.0").is_ok(), "{english}");
        }
    }

    #[test]
    fn todo_as_a_whole_word_is_still_rejected() {
        let message = error_text(&section("0.1.0", "Fix: TODO later", "Vi"), "0.1.0");
        assert!(message.contains("todo"), "{message}");
    }

    #[test]
    fn coming_soon_phrase_is_rejected() {
        let message = error_text(&section("0.1.0", "Export is Coming Soon", "Vi"), "0.1.0");
        assert!(message.contains("coming soon"), "{message}");
    }
}
