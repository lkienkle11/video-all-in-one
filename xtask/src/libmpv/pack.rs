//! `cargo xtask libmpv pack`: the folder of a finished build as one `.tar.gz` and its SHA-256.

use super::fetch::sha256_file;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// Makes the archive `out` (`.tar.gz`) from the folder `dir` and returns its SHA-256. The folder
/// must be a finished build: it needs `lib`, `include` and `MANIFEST.txt`.
pub(super) fn pack(dir: &Path, out: &Path) -> Result<String> {
    let name = out
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !name.ends_with(".tar.gz") {
        bail!("the archive name must end in .tar.gz, not {name:?}");
    }
    for needed in ["lib", "include", "MANIFEST.txt"] {
        if !dir.join(needed).exists() {
            bail!(
                "{} is not a finished libmpv build: {needed} is missing",
                dir.display()
            );
        }
    }
    if out.exists() {
        bail!(
            "{} exists already; this command never overwrites an archive",
            out.display()
        );
    }
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    // Absolute path: tar runs inside `dir`, where a relative `out` would point elsewhere.
    let out = std::path::absolute(out).context("resolving the archive path")?;
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&out)
        .arg("-C")
        .arg(dir)
        .arg(".")
        .status()
        .context("starting tar")?;
    if !status.success() {
        bail!("tar failed with {status}");
    }
    sha256_file(&out)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vaio-pack-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn build_folder(dir: &Path) {
        std::fs::create_dir_all(dir.join("lib")).unwrap();
        std::fs::create_dir_all(dir.join("include")).unwrap();
        std::fs::write(dir.join("lib/libmpv.so.2"), b"lib").unwrap();
        std::os::unix::fs::symlink("libmpv.so.2", dir.join("lib/libmpv.so")).unwrap();
        std::fs::write(dir.join("MANIFEST.txt"), b"manifest").unwrap();
    }

    #[test]
    fn the_archive_holds_the_folder_and_its_hash_matches() {
        let root = scratch("ok");
        let dir = root.join("libmpv");
        build_folder(&dir);
        let out = root.join("archives/libmpv-1-test.tar.gz");
        let hash = pack(&dir, &out).unwrap();
        assert_eq!(hash, sha256_file(&out).unwrap());
        assert_eq!(hash.len(), 64);
        let listing = Command::new("tar").arg("-tzf").arg(&out).output().unwrap();
        let listing = String::from_utf8_lossy(&listing.stdout).into_owned();
        for wanted in ["./lib/libmpv.so.2", "./lib/libmpv.so", "./MANIFEST.txt"] {
            assert!(
                listing.lines().any(|l| l == wanted),
                "{wanted} in\n{listing}"
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_folder_that_is_not_a_build_or_a_wrong_name_is_refused() {
        let root = scratch("bad");
        let empty = root.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let error = pack(&empty, &root.join("a.tar.gz"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("not a finished libmpv build"), "{error}");
        let dir = root.join("libmpv");
        build_folder(&dir);
        assert!(pack(&dir, &root.join("a.zip")).is_err());
        let out = root.join("a.tar.gz");
        pack(&dir, &out).unwrap();
        let again = pack(&dir, &out).unwrap_err().to_string();
        assert!(again.contains("never overwrites"), "{again}");
        std::fs::remove_dir_all(root).unwrap();
    }
}
