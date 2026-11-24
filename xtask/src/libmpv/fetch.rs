//! Downloading and unpacking the pinned source archives. Every archive is hashed and compared
//! with the pinned SHA-256 BEFORE anything is unpacked, so a tampered or truncated download
//! never reaches `tar`.

use super::sources::Source;
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The lowercase hex SHA-256 of a file, read in chunks.
pub(super) fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0u8; 1 << 20];
    loop {
        let read = file
            .read(&mut chunk)
            .with_context(|| format!("reading {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&chunk[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Fails unless `path` has the SHA-256 `expected`.
pub(super) fn verify_hash(path: &Path, expected: &str) -> Result<()> {
    let actual = sha256_file(path)?;
    if actual != expected {
        bail!(
            "{} has SHA-256 {actual}, but {expected} is pinned: refusing to use it",
            path.display()
        );
    }
    Ok(())
}

/// Downloads `source` into `cache` unless a file with the pinned hash is already there, and
/// returns its path. A download with a wrong hash is deleted.
pub(super) fn fetch(source: &Source, cache: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(cache).with_context(|| format!("creating {}", cache.display()))?;
    let file = cache.join(source.file);
    if file.is_file() && verify_hash(&file, source.sha256).is_ok() {
        return Ok(file);
    }
    let partial = cache.join(format!("{}.part", source.file));
    println!("downloading {} {}", source.name, source.version);
    let status = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error"])
        .args(["--proto", "=https", "--tlsv1.2", "--retry", "3", "--output"])
        .arg(&partial)
        .arg(source.url)
        .status()
        .context("starting curl")?;
    if !status.success() {
        bail!("curl failed for {} ({status})", source.url);
    }
    if let Err(error) = verify_hash(&partial, source.sha256) {
        let _ = std::fs::remove_file(&partial);
        return Err(error);
    }
    std::fs::rename(&partial, &file).with_context(|| format!("moving into {}", file.display()))?;
    Ok(file)
}

/// Verifies `archive` against `expected`, then unpacks it into the empty folder `dest`, without
/// the archive's top folder. Nothing is created when the hash is wrong.
pub(super) fn unpack_verified(archive: &Path, expected: &str, dest: &Path) -> Result<()> {
    verify_hash(archive, expected)?;
    if dest.exists() {
        std::fs::remove_dir_all(dest).with_context(|| format!("clearing {}", dest.display()))?;
    }
    std::fs::create_dir_all(dest).with_context(|| format!("creating {}", dest.display()))?;
    let status = Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .arg("--strip-components=1")
        .status()
        .context("starting tar")?;
    if !status.success() {
        bail!("tar could not unpack {} ({status})", archive.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaio-fetch-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// SHA-256 of the three bytes `abc`.
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn sha256_of_a_small_file_is_the_known_digest() {
        let dir = scratch("known");
        let file = dir.join("abc");
        std::fs::write(&file, b"abc").unwrap();
        assert_eq!(sha256_file(&file).unwrap(), ABC);
        verify_hash(&file, ABC).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_wrong_hash_is_rejected_before_anything_is_unpacked() {
        let dir = scratch("wrong");
        let archive = dir.join("a.tar");
        std::fs::write(&archive, b"not even a tar file").unwrap();
        let dest = dir.join("src");
        let wrong = "0".repeat(64);
        let error = unpack_verified(&archive, &wrong, &dest)
            .unwrap_err()
            .to_string();
        assert!(error.contains("refusing to use it"), "{error}");
        assert!(error.contains(&wrong), "{error}");
        assert!(
            !dest.exists(),
            "nothing may be unpacked after a hash failure"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_right_hash_unpacks_without_the_top_folder() {
        let dir = scratch("right");
        let tree = dir.join("pkg-1.0");
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::write(tree.join("COPYING"), b"licence").unwrap();
        let archive = dir.join("pkg-1.0.tar");
        let status = Command::new("tar")
            .arg("-cf")
            .arg(&archive)
            .arg("-C")
            .arg(&dir)
            .arg("pkg-1.0")
            .status()
            .unwrap();
        assert!(status.success());
        let hash = sha256_file(&archive).unwrap();
        let dest = dir.join("src");
        unpack_verified(&archive, &hash, &dest).unwrap();
        assert_eq!(std::fs::read(dest.join("COPYING")).unwrap(), b"licence");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
