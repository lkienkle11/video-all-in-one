use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct MapEntry {
    pub path: PathBuf,
    pub purpose: String,
}

const START: &str = "<!-- structure-map:start -->";
const END: &str = "<!-- structure-map:end -->";

fn bounds(markdown: &str) -> anyhow::Result<(usize, usize)> {
    let start = markdown
        .find(START)
        .ok_or_else(|| anyhow::anyhow!("missing map start"))?
        + START.len();
    let end = markdown[start..]
        .find(END)
        .ok_or_else(|| anyhow::anyhow!("missing map end"))?
        + start;
    Ok((start, end))
}

pub fn parse_map(markdown: &str) -> anyhow::Result<Vec<MapEntry>> {
    let (start, end) = bounds(markdown)?;
    markdown[start..end]
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (path, purpose) = line
                .split_once('|')
                .ok_or_else(|| anyhow::anyhow!("invalid map entry: {line}"))?;
            let path = path.trim();
            anyhow::ensure!(!path.is_empty(), "empty map path");
            Ok(MapEntry {
                path: path.into(),
                purpose: purpose.trim().into(),
            })
        })
        .collect()
}

pub fn render(markdown: &str, directories: &[PathBuf]) -> anyhow::Result<String> {
    let entries = parse_map(markdown)?;
    let (start, end) = bounds(markdown)?;
    let mut output = markdown[..start].to_owned();
    output.push('\n');
    for path in directories {
        let purpose = entries
            .iter()
            .find(|e| &e.path == path)
            .map_or("TODO", |e| e.purpose.as_str());
        output.push_str(&format!("{} | {}\n", path.display(), purpose));
    }
    output.push_str(&markdown[end..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_map_trims_path_and_purpose() {
        let entries = parse_map(
            "<!-- structure-map:start -->\n a/b  |  useful purpose  \n<!-- structure-map:end -->",
        )
        .unwrap();
        assert_eq!(entries[0].path, PathBuf::from("a/b"));
        assert_eq!(entries[0].purpose, "useful purpose");
    }
    #[test]
    fn parse_map_rejects_empty_path() {
        assert!(
            parse_map("<!-- structure-map:start -->\n | purpose\n<!-- structure-map:end -->")
                .is_err()
        );
    }
    #[test]
    fn write_preserves_text_outside_markers() {
        let text="intro | unchanged\n<!-- structure-map:start -->\na | old\n<!-- structure-map:end -->\noutro | unchanged\n";
        let result = render(text, &["b".into()]).unwrap();
        let (start, end) = bounds(text).unwrap();
        let (new_start, new_end) = bounds(&result).unwrap();
        assert_eq!(&text[..start], &result[..new_start]);
        assert_eq!(&text[end..], &result[new_end..]);
    }
    #[test]
    fn parse_map_rejects_missing_end_marker() {
        assert!(parse_map("<!-- structure-map:start -->\na | purpose").is_err());
    }
    #[test]
    fn parse_map_rejects_missing_start_marker() {
        assert!(parse_map("a | purpose\n<!-- structure-map:end -->").is_err());
    }
    #[test]
    fn parse_map_rejects_line_without_pipe() {
        assert!(
            parse_map("<!-- structure-map:start -->\nbad\n<!-- structure-map:end -->").is_err()
        );
    }
    #[test]
    fn parse_map_reads_entries_and_ignores_comments_and_outside() {
        let text="outside | ignored\n<!-- structure-map:start -->\n\n# comment\na | A purpose.\nb | B purpose.\n<!-- structure-map:end -->\noutside | ignored";
        let entries = parse_map(text).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, PathBuf::from("a"));
        assert_eq!(entries[0].purpose, "A purpose.");
        assert_eq!(entries[1].path, PathBuf::from("b"));
        assert_eq!(entries[1].purpose, "B purpose.");
    }
    #[test]
    fn write_keeps_existing_purposes_and_adds_todo_for_new_directory() {
        let text = "before\n<!-- structure-map:start -->\na | Existing purpose.\n<!-- structure-map:end -->\nafter\n";
        let result = render(text, &["a".into(), "b".into()]).unwrap();
        assert!(result.contains("a | Existing purpose."));
        assert!(result.contains("b | TODO"));
        assert!(result.starts_with("before\n"));
        assert!(result.ends_with("after\n"));
        assert_eq!(parse_map(&result).unwrap().len(), 2);
    }
}
