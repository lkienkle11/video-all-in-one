use super::{add, Rule, Violation};
use std::path::{Path, PathBuf};

pub(super) fn check(root: &Path, dirs: &[PathBuf], out: &mut Vec<Violation>) {
    let mut source_dirs: Vec<_> = dirs
        .iter()
        .filter(|d| d.components().any(|c| c.as_os_str() == "src"))
        .cloned()
        .collect();
    source_dirs.extend(dirs.iter().filter_map(|d| {
        let src = d.join("src");
        root.join(&src).is_dir().then_some(src)
    }));
    source_dirs.sort();
    source_dirs.dedup();
    for dir in source_dirs {
        let forbidden = |s: &str| {
            matches!(
                s.to_ascii_lowercase().as_str(),
                "utils" | "helpers" | "common" | "misc" | "shared" | "manager" | "stuff"
            )
        };
        if dir
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(forbidden)
        {
            add(out, Rule::ForbiddenName, &dir, "forbidden directory name");
        }
        if let Ok(entries) = std::fs::read_dir(root.join(&dir)) {
            let mut count = 0;
            for entry in entries.flatten() {
                let path = entry.path();
                if entry.file_type().is_ok_and(|t| t.is_file())
                    && path.extension().is_some_and(|e| e == "rs")
                {
                    count += 1;
                    let rel = dir.join(entry.file_name());
                    if path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(forbidden)
                    {
                        add(out, Rule::ForbiddenName, &rel, "forbidden file name");
                    }
                    if let Ok(bytes) = std::fs::read(&path) {
                        let lines = bytes.iter().filter(|&&b| b == b'\n').count()
                            + usize::from(!bytes.is_empty() && bytes.last() != Some(&b'\n'));
                        if lines > 400 {
                            add(out, Rule::FileTooLong, rel, format!("{lines} lines"));
                        }
                    }
                }
            }
            if count > 12 {
                add(
                    out,
                    Rule::DirectoryTooLarge,
                    dir,
                    format!("{count} Rust files"),
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
    fn directory_with_13_source_files_is_a_violation() {
        let root = fixture();
        for i in 0..13 {
            fs::write(root.join(format!("crates/core/src/f{i}.rs")), "").unwrap();
        }
        assert!(has(
            &root,
            &metadata("vaio-core", &[]),
            &map(),
            Rule::DirectoryTooLarge
        ));
    }
    #[test]
    fn file_at_exactly_400_lines_is_allowed() {
        let root = fixture();
        fs::write(root.join("crates/core/src/long.rs"), "x\n".repeat(400)).unwrap();
        assert!(!has(
            &root,
            &metadata("vaio-core", &[]),
            &map(),
            Rule::FileTooLong
        ));
    }
    #[test]
    fn file_over_400_lines_is_a_violation() {
        let root = fixture();
        fs::write(root.join("crates/core/src/long.rs"), "x\n".repeat(401)).unwrap();
        assert!(has(
            &root,
            &metadata("vaio-core", &[]),
            &map(),
            Rule::FileTooLong
        ));
    }
    #[test]
    fn module_dir_named_helpers_is_a_violation() {
        let root = fixture();
        fs::create_dir(root.join("crates/core/src/helpers")).unwrap();
        assert!(has(
            &root,
            &metadata("vaio-core", &[]),
            &map(),
            Rule::ForbiddenName
        ));
    }
    #[test]
    fn file_named_utils_rs_is_a_violation() {
        let root = fixture();
        fs::write(root.join("crates/core/src/utils.rs"), "").unwrap();
        assert!(has(
            &root,
            &metadata("vaio-core", &[]),
            &map(),
            Rule::ForbiddenName
        ));
    }
}
