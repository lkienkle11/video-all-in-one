use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

pub fn run(root: &Path, min: f64) -> Result<()> {
    let share = rust_share(root)?;
    println!("Rust: {share:.1}%");
    check_minimum(share, min)
}

pub fn rust_share(root: &Path) -> Result<f64> {
    fn count(path: &Path, rust: &mut usize, total: &mut usize) -> Result<()> {
        for entry in fs::read_dir(path).with_context(|| format!("reading {}", path.display()))? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                let name = entry.file_name();
                if !name.to_string_lossy().starts_with('.')
                    && !matches!(name.to_str(), Some("target" | "docs" | "node_modules"))
                {
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
                    let bytes =
                        fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
                    let lines = bytes.iter().filter(|&&byte| byte == b'\n').count()
                        + usize::from(!bytes.is_empty() && bytes.last() != Some(&b'\n'));
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
    if !minimum.is_finite() || !(0.0..=100.0).contains(&minimum) {
        bail!("minimum must be a finite number between 0 and 100");
    }
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

    struct Fixture(PathBuf);

    impl std::ops::Deref for Fixture {
        type Target = std::path::Path;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> Fixture {
        let path = std::env::temp_dir().join(format!(
            "vaio-lang-share-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Fixture(path)
    }

    #[test]
    fn skips_every_directory_whose_name_starts_with_a_dot() {
        let root = fixture();
        fs::write(root.join("code.rs"), "one\n").unwrap();
        for name in [".hidden", ".git"] {
            fs::create_dir(root.join(name)).unwrap();
            fs::write(root.join(name).join("code.sh"), "one\n").unwrap();
        }
        assert_eq!(rust_share(&root).unwrap(), 100.0);
    }

    #[test]
    fn non_utf8_source_file_is_counted_by_newlines_and_does_not_abort() {
        let root = fixture();
        fs::write(root.join("bad.rs"), [0xff, 0xfe, b'\n', b'x', b'\n']).unwrap();
        fs::write(root.join("ok.sh"), "one\ntwo\n").unwrap();
        assert_eq!(rust_share(&root).unwrap(), 50.0);
    }

    #[test]
    fn minimum_must_be_a_finite_number_between_0_and_100() {
        for minimum in [f64::NAN, -1.0, 101.0] {
            let error = check_minimum(80.0, minimum).unwrap_err().to_string();
            assert!(error.contains("0") && error.contains("100"), "{error}");
        }
        assert!(check_minimum(80.0, 80.0).is_ok());
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
        assert!(super::run(&root, 85.0).is_err());
        assert!(super::run(&root, 60.0).is_ok());
    }
}
