use anyhow::{bail, Context, Result};
use semver::Version;
use std::fs;
use std::path::Path;

/// Rewrites the workspace version in `Cargo.toml` and every workspace member in
/// `Cargo.lock`, then checks the result before writing either file.
pub fn set_version(root: &Path, version: &str) -> Result<()> {
    plain_version(version)?;
    let cargo_path = root.join("Cargo.toml");
    let lock_path = root.join("Cargo.lock");
    let cargo = read(&cargo_path)?;
    let lock = read(&lock_path)?;

    let new_cargo = replace_workspace_version(&cargo, version)?;
    let new_lock = replace_member_versions(&lock, version)?;

    let mut mismatches = Vec::new();
    let workspace = workspace_version(&new_cargo)?;
    if workspace != version {
        mismatches.push(format!("Cargo.toml has {workspace}"));
    }
    for (name, found) in lock_members(&new_lock)? {
        if found != version {
            mismatches.push(format!("Cargo.lock package `{name}` has {found}"));
        }
    }
    if !mismatches.is_empty() {
        bail!("versions do not equal {version}: {}", mismatches.join("; "));
    }

    fs::write(&cargo_path, new_cargo)
        .with_context(|| format!("writing {}", cargo_path.display()))?;
    fs::write(&lock_path, new_lock).with_context(|| format!("writing {}", lock_path.display()))?;
    Ok(())
}

/// Parses a release version: three numeric parts, no pre-release, no build metadata.
pub fn plain_version(text: &str) -> Result<Version> {
    let version =
        Version::parse(text).with_context(|| format!("`{text}` is not a plain X.Y.Z version"))?;
    if !version.pre.is_empty() || !version.build.is_empty() {
        bail!("`{text}` is not a plain X.Y.Z version (no pre-release or build metadata)");
    }
    Ok(version)
}

/// Reads `version` under `[workspace.package]` in a `Cargo.toml` text.
pub fn workspace_version(cargo: &str) -> Result<String> {
    let index = workspace_version_line(cargo)?;
    let line = cargo.lines().nth(index).unwrap_or_default();
    Ok(quoted(line.trim_start_matches("version = ")).to_string())
}

/// Lists `(name, version)` of every workspace member in a `Cargo.lock` text (a package
/// block without a `source` line). A member block without a version line is an error.
pub fn lock_members(lock: &str) -> Result<Vec<(String, String)>> {
    let lines: Vec<&str> = lock.split('\n').collect();
    let mut members = Vec::new();
    for member in member_blocks(&lines) {
        let Some(index) = member.version_line else {
            bail!(
                "package `{}` in Cargo.lock has no version line",
                member.name
            );
        };
        let version = quoted(lines[index].trim_start_matches("version = "));
        members.push((member.name, version.to_string()));
    }
    Ok(members)
}

fn replace_workspace_version(cargo: &str, version: &str) -> Result<String> {
    let index = workspace_version_line(cargo)?;
    let mut lines: Vec<String> = cargo.split('\n').map(str::to_string).collect();
    lines[index] = format!("version = \"{version}\"");
    Ok(lines.join("\n"))
}

fn replace_member_versions(lock: &str, version: &str) -> Result<String> {
    let mut lines: Vec<String> = lock.split('\n').map(str::to_string).collect();
    let members = member_blocks(&lines.iter().map(String::as_str).collect::<Vec<_>>());
    for member in members {
        let Some(index) = member.version_line else {
            bail!(
                "package `{}` in Cargo.lock has no version line; Cargo.lock was not changed",
                member.name
            );
        };
        lines[index] = format!("version = \"{version}\"");
    }
    Ok(lines.join("\n"))
}

struct MemberBlock {
    name: String,
    version_line: Option<usize>,
}

#[derive(Default)]
struct PackageBlock {
    name: String,
    version_line: Option<usize>,
    has_source: bool,
}

/// Workspace members: package blocks without a `source` line.
fn member_blocks(lines: &[&str]) -> Vec<MemberBlock> {
    let mut blocks: Vec<PackageBlock> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.trim_end() == "[[package]]" {
            blocks.push(PackageBlock::default());
            continue;
        }
        let Some(block) = blocks.last_mut() else {
            continue;
        };
        if let Some(value) = line.strip_prefix("name = ") {
            block.name = quoted(value).to_string();
        } else if line.starts_with("version = ") && block.version_line.is_none() {
            block.version_line = Some(index);
        } else if line.starts_with("source = ") {
            block.has_source = true;
        }
    }
    blocks
        .into_iter()
        .filter(|block| !block.has_source)
        .map(|block| MemberBlock {
            name: block.name,
            version_line: block.version_line,
        })
        .collect()
}

fn workspace_version_line(cargo: &str) -> Result<usize> {
    let mut in_section = false;
    for (index, line) in cargo.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == "[workspace.package]";
        } else if in_section && line.starts_with("version = ") {
            return Ok(index);
        }
    }
    bail!("Cargo.toml has no version under [workspace.package]")
}

fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

fn quoted(value: &str) -> &str {
    value.trim().trim_matches('"')
}

#[cfg(test)]
pub(crate) mod fixture {
    use std::fs;
    use std::ops::Deref;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    pub(crate) struct Fixture(PathBuf);

    impl Fixture {
        pub(crate) fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "vaio-xtask-version-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Fixture(path)
        }
    }

    impl Deref for Fixture {
        type Target = Path;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fixture::Fixture, set_version};
    use std::fs;

    const CARGO: &str = "[workspace]\nmembers = [\"a\", \"b\", \"c\"]\n\n[workspace.package]\nversion = \"0.0.0\"\nedition = \"2021\"\n";

    const LOCK: &str = "version = 4\n\n[[package]]\nname = \"a\"\nversion = \"0.0.0\"\n\n[[package]]\nname = \"anyhow\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"aaaa\"\n\n[[package]]\nname = \"b\"\nversion = \"0.0.0\"\n\n[[package]]\nname = \"c\"\nversion = \"0.0.0\"\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"bbbb\"\n";

    const LOCK_MEMBER_WITHOUT_VERSION: &str =
        "version = 4\n\n[[package]]\nname = \"a\"\n\n[[package]]\nname = \"b\"\nversion = \"0.0.0\"\n";

    fn write_repo(root: &Fixture, cargo: &str, lock: &str) {
        fs::write(root.join("Cargo.toml"), cargo).unwrap();
        fs::write(root.join("Cargo.lock"), lock).unwrap();
    }

    fn read(root: &Fixture, name: &str) -> String {
        fs::read_to_string(root.join(name)).unwrap()
    }

    #[test]
    fn set_version_updates_every_file_and_they_agree() {
        let root = Fixture::new();
        write_repo(&root, CARGO, LOCK);

        set_version(&root, "0.1.0").unwrap();

        assert_eq!(
            read(&root, "Cargo.toml"),
            CARGO.replace("version = \"0.0.0\"", "version = \"0.1.0\"")
        );
        assert_eq!(
            read(&root, "Cargo.lock"),
            LOCK.replace("version = \"0.0.0\"", "version = \"0.1.0\"")
        );
    }

    #[test]
    fn set_version_rejects_invalid_semver() {
        for bad in ["v0.1.0", "0.1", "0.1.0-alpha.1", "0.1.0+b5", ""] {
            let root = Fixture::new();
            write_repo(&root, CARGO, LOCK);

            assert!(set_version(&root, bad).is_err(), "accepted {bad:?}");
            assert_eq!(
                read(&root, "Cargo.toml"),
                CARGO,
                "Cargo.toml changed for {bad:?}"
            );
            assert_eq!(
                read(&root, "Cargo.lock"),
                LOCK,
                "Cargo.lock changed for {bad:?}"
            );
        }
    }

    #[test]
    fn set_version_fails_when_a_member_block_cannot_be_updated() {
        let root = Fixture::new();
        write_repo(&root, CARGO, LOCK_MEMBER_WITHOUT_VERSION);

        assert!(set_version(&root, "0.1.0").is_err());
        assert_eq!(read(&root, "Cargo.toml"), CARGO);
        assert_eq!(read(&root, "Cargo.lock"), LOCK_MEMBER_WITHOUT_VERSION);
    }
}
