use super::{add, Rule, Violation};
use std::path::Path;

pub(super) fn check(root: &Path, out: &mut Vec<Violation>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().is_ok_and(|t| t.is_dir())
                && !name.starts_with('.')
                && !matches!(
                    name.as_str(),
                    "target"
                        | "apps"
                        | "crates"
                        | "xtask"
                        | "docs"
                        | "assets"
                        | "resources"
                        | "tests"
                        | "scripts"
                        | "spikes"
                )
            {
                add(
                    out,
                    Rule::UnknownTopLevelDir,
                    name,
                    "unknown top-level directory",
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{fixture, has, map, metadata};
    use super::super::Rule;
    use std::fs;
    #[test]
    fn unknown_top_level_dir_is_a_violation() {
        let root = fixture();
        fs::create_dir(root.join("strange")).unwrap();
        assert!(has(
            &root,
            &metadata("vaio-core", &[]),
            &map(),
            Rule::UnknownTopLevelDir
        ));
    }
}
