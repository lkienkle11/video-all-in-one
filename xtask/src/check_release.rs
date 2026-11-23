use crate::set_version::{lock_members, plain_version, workspace_version};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

/// Checks that `tag` (`vX.Y.Z`) matches the workspace version and every workspace member
/// in `Cargo.lock`, and that it is greater than `latest` (`none` for the first release).
pub fn check_release(root: &Path, tag: &str, latest: &str) -> Result<()> {
    let Some(tag_text) = tag.strip_prefix('v') else {
        bail!("tag `{tag}` must be v followed by X.Y.Z");
    };
    let tag_version = plain_version(tag_text)
        .with_context(|| format!("tag `{tag}` is not v followed by X.Y.Z"))?;

    let cargo_path = root.join("Cargo.toml");
    let cargo = fs::read_to_string(&cargo_path)
        .with_context(|| format!("reading {}", cargo_path.display()))?;
    let workspace = workspace_version(&cargo)?;
    if workspace != tag_version.to_string() {
        bail!(
            "tag {tag} names version {tag_version} but Cargo.toml workspace version is {workspace}"
        );
    }

    let lock_path = root.join("Cargo.lock");
    let lock = fs::read_to_string(&lock_path)
        .with_context(|| format!("reading {}", lock_path.display()))?;
    let mismatches: Vec<String> = lock_members(&lock)?
        .into_iter()
        .filter(|(_, version)| *version != tag_version.to_string())
        .map(|(name, version)| format!("`{name}` has {version}"))
        .collect();
    if !mismatches.is_empty() {
        bail!(
            "Cargo.lock workspace members do not equal {tag_version}: {}",
            mismatches.join(", ")
        );
    }

    if latest != "none" {
        let latest_version = plain_version(latest)
            .with_context(|| format!("--latest `{latest}` must be none or X.Y.Z"))?;
        if tag_version <= latest_version {
            bail!("version {tag_version} must be greater than the latest release {latest_version}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_release;
    use crate::set_version::fixture::Fixture;
    use std::fs;

    fn repo(workspace: &str, member_b: &str) -> Fixture {
        let root = Fixture::new();
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "[workspace]\nmembers = [\"a\", \"b\"]\n\n[workspace.package]\nversion = \"{workspace}\"\nedition = \"2021\"\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("Cargo.lock"),
            format!(
                "version = 4\n\n[[package]]\nname = \"a\"\nversion = \"{workspace}\"\n\n[[package]]\nname = \"b\"\nversion = \"{member_b}\"\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"bbbb\"\n"
            ),
        )
        .unwrap();
        root
    }

    #[test]
    fn check_release_rejects_tag_not_matching_files() {
        let root = repo("0.0.0", "0.0.0");
        let error = check_release(&root, "v0.1.0", "none")
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("0.1.0") && error.contains("0.0.0"),
            "{error}"
        );
    }

    #[test]
    fn check_release_rejects_non_increasing_version() {
        let root = repo("0.1.0", "0.1.0");
        assert!(check_release(&root, "v0.1.0", "0.1.0").is_err());
        assert!(check_release(&root, "v0.1.0", "0.2.0").is_err());
    }

    #[test]
    fn check_release_accepts_a_higher_version_and_the_first_release() {
        let higher = repo("0.1.0", "0.1.0");
        assert!(check_release(&higher, "v0.1.0", "0.0.9").is_ok());

        let first = repo("0.0.0", "0.0.0");
        assert!(check_release(&first, "v0.0.0", "none").is_ok());
    }

    #[test]
    fn check_release_rejects_a_tag_without_v_or_with_prerelease() {
        let root = repo("0.1.0", "0.1.0");
        assert!(check_release(&root, "0.1.0", "none").is_err());
        assert!(check_release(&root, "v0.1.0-rc.1", "none").is_err());
        assert!(check_release(&root, "v0.1.0", "0.1.0-alpha").is_err());
    }

    #[test]
    fn check_release_rejects_a_lock_file_member_with_a_different_version() {
        let root = repo("0.1.0", "0.0.0");
        let error = check_release(&root, "v0.1.0", "none")
            .unwrap_err()
            .to_string();
        assert!(error.contains("b"), "{error}");
    }
}
