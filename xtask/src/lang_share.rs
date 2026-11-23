use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

pub fn rust_share(root: &Path) -> Result<f64> {
    fn count(path: &Path, rust: &mut usize, total: &mut usize) -> Result<()> {
        for entry in fs::read_dir(path).with_context(|| format!("reading {}", path.display()))? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                if !matches!(
                    entry.file_name().to_str(),
                    Some("target" | "docs" | ".claude" | ".agents" | ".codegraph" | "node_modules")
                ) {
                    count(&entry.path(), rust, total)?;
                }
            } else if file_type.is_file() {
                let path = entry.path();
                let extension = path.extension().and_then(|ext| ext.to_str());
                if matches!(
                    extension,
                    Some(
                        "rs" | "sh"
                            | "js"
                            | "ts"
                            | "py"
                            | "c"
                            | "cpp"
                            | "h"
                            | "swift"
                            | "kt"
                            | "java"
                            | "rb"
                    )
                ) {
                    let lines = fs::read_to_string(&path)
                        .with_context(|| format!("reading {}", path.display()))?
                        .lines()
                        .count();
                    *total += lines;
                    if extension == Some("rs") {
                        *rust += lines;
                    }
                }
            }
        }
        Ok(())
    }

    let (mut rust, mut total) = (0, 0);
    count(root, &mut rust, &mut total)?;
    Ok(if total == 0 {
        100.0
    } else {
        rust as f64 * 100.0 / total as f64
    })
}

pub fn check_minimum(share: f64, minimum: f64) -> Result<()> {
    if share < minimum {
        bail!("Rust share {share:.1}% is below {minimum:.1}%");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check_minimum, rust_share};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn fixture() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "vaio-lang-share-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn counts_only_code_extensions() {
        let root = fixture();
        fs::write(root.join("code.rs"), "one\ntwo\n").unwrap();
        fs::write(root.join("code.sh"), "one\n").unwrap();
        fs::write(root.join("notes.txt"), "one\ntwo\nthree\n").unwrap();
        assert!((rust_share(&root).unwrap() - 200.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn skips_ignored_directories() {
        let root = fixture();
        fs::write(root.join("code.rs"), "one\n").unwrap();
        for name in [
            "target",
            "docs",
            ".claude",
            ".agents",
            ".codegraph",
            "node_modules",
        ] {
            fs::create_dir(root.join(name)).unwrap();
            fs::write(root.join(name).join("code.sh"), "one\n").unwrap();
        }
        assert_eq!(rust_share(&root).unwrap(), 100.0);
    }

    #[test]
    fn returns_one_hundred_percent_when_only_rust() {
        let root = fixture();
        fs::write(root.join("code.rs"), "one\ntwo\n").unwrap();
        assert_eq!(rust_share(&root).unwrap(), 100.0);
    }

    #[test]
    fn fails_below_minimum() {
        let root = fixture();
        fs::write(root.join("code.rs"), "line\n".repeat(10)).unwrap();
        fs::write(root.join("code.sh"), "line\n".repeat(5)).unwrap();
        let share = rust_share(&root).unwrap();
        assert!((share - 66.7).abs() < 0.1);
        assert!(check_minimum(share, 85.0).is_err());
    }

    #[test]
    fn command_rejects_and_accepts_minimum_for_fixture() {
        let root = fixture();
        fs::write(root.join("code.rs"), "line\n".repeat(10)).unwrap();
        fs::write(root.join("code.sh"), "line\n".repeat(5)).unwrap();
        assert!(crate::run_lang_share(&root, 85.0).is_err());
        assert!(crate::run_lang_share(&root, 60.0).is_ok());
    }
}
