mod dependencies;
mod map;
mod source;
mod top_level;

use crate::structure_map::MapEntry;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum Rule {
    ForbiddenCrateDependency,
    ForbiddenName,
    FileTooLong,
    DirectoryTooLarge,
    UnknownTopLevelDir,
    UndocumentedDirectory,
    StaleMapEntry,
    MissingPurpose,
}
#[derive(Debug)]
pub struct Violation {
    pub rule: Rule,
    pub path: PathBuf,
    pub detail: String,
}
fn add(out: &mut Vec<Violation>, rule: Rule, path: impl Into<PathBuf>, detail: impl Into<String>) {
    out.push(Violation {
        rule,
        path: path.into(),
        detail: detail.into(),
    });
}

pub fn directories(root: &Path) -> Vec<PathBuf> {
    fn descend(root: &Path, rel: &Path, dirs: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(root.join(rel)) {
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|t| t.is_dir()) {
                    let path = rel.join(entry.file_name());
                    dirs.push(path.clone());
                    descend(root, &path, dirs);
                }
            }
        }
    }
    let mut dirs = Vec::new();
    for parent in ["crates", "apps"] {
        if let Ok(entries) = std::fs::read_dir(root.join(parent)) {
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|t| t.is_dir()) {
                    let dir = PathBuf::from(parent).join(entry.file_name());
                    dirs.push(dir.clone());
                    descend(root, &dir.join("src"), &mut dirs);
                }
            }
        }
    }
    if root.join("xtask").is_dir() {
        dirs.push("xtask".into());
        descend(root, Path::new("xtask/src"), &mut dirs);
    }
    dirs.sort();
    dirs
}

pub fn check(root: &Path, metadata_json: &str, map: &[MapEntry]) -> Vec<Violation> {
    let mut out = Vec::new();
    if !dependencies::check(metadata_json, &mut out) {
        return out;
    }
    let dirs = directories(root);
    self::map::check(&dirs, map, &mut out);
    top_level::check(root, &mut out);
    source::check(root, &dirs, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    pub(super) fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "structure-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("crates/core/src")).unwrap();
        root
    }
    pub(super) fn map() -> Vec<MapEntry> {
        vec![MapEntry {
            path: "crates/core".into(),
            purpose: "Domain policy.".into(),
        }]
    }
    pub(super) fn metadata(name: &str, dependencies: &[&str]) -> String {
        let deps: Vec<_> = dependencies
            .iter()
            .map(|d| serde_json::json!({"name": d}))
            .collect();
        serde_json::json!({"workspace_members": ["id"], "packages": [{"id":"id", "name":name, "dependencies":deps}]}).to_string()
    }
    pub(super) fn has(root: &Path, meta: &str, entries: &[MapEntry], rule: Rule) -> bool {
        check(root, meta, entries).iter().any(|v| v.rule == rule)
    }
}
