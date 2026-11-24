use super::{add, MapEntry, Rule, Violation};
use std::path::PathBuf;

pub(super) fn check(dirs: &[PathBuf], map: &[MapEntry], out: &mut Vec<Violation>) {
    for dir in dirs {
        if !map.iter().any(|e| &e.path == dir) {
            add(
                out,
                Rule::UndocumentedDirectory,
                dir,
                "directory is absent from map",
            );
        }
    }
    for entry in map {
        if !dirs.contains(&entry.path) {
            add(
                out,
                Rule::StaleMapEntry,
                &entry.path,
                "directory is absent from tree",
            );
        }
        if entry.purpose.trim().is_empty() || entry.purpose.trim().eq_ignore_ascii_case("todo") {
            add(out, Rule::MissingPurpose, &entry.path, "purpose is missing");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{fixture, has, map, metadata};
    use super::super::{check, MapEntry, Rule};
    use std::fs;
    #[test]
    fn clean_tree_with_complete_map_has_no_violations() {
        let root = fixture();
        assert!(check(&root, &metadata("vaio-core", &[]), &map()).is_empty());
    }
    #[test]
    fn todo_purpose_is_missing_purpose() {
        for purpose in ["TODO", "todo", "ToDo", ""] {
            let root = fixture();
            let entries = vec![MapEntry {
                path: "crates/core".into(),
                purpose: purpose.into(),
            }];
            assert!(has(
                &root,
                &metadata("vaio-core", &[]),
                &entries,
                Rule::MissingPurpose
            ));
        }
    }
    #[test]
    fn map_entry_for_deleted_directory_is_stale() {
        let root = fixture();
        let mut entries = map();
        entries.push(MapEntry {
            path: "crates/core/src/deleted".into(),
            purpose: "Old.".into(),
        });
        assert!(has(
            &root,
            &metadata("vaio-core", &[]),
            &entries,
            Rule::StaleMapEntry
        ));
    }
    #[test]
    fn new_directory_missing_from_map_is_undocumented() {
        let root = fixture();
        fs::create_dir(root.join("crates/core/src/domain")).unwrap();
        assert!(has(
            &root,
            &metadata("vaio-core", &[]),
            &map(),
            Rule::UndocumentedDirectory
        ));
    }
}
