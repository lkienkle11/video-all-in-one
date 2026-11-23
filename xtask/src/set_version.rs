use anyhow::{bail, Context, Result};
use semver::Version;
use std::fs;
use std::path::{Path, PathBuf};

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

    write_atomic(&cargo_path, &new_cargo)?;
    if let Err(error) = write_atomic(&lock_path, &new_lock) {
        return match write_atomic(&cargo_path, &cargo) {
            Ok(()) => Err(error.context(format!(
                "{} was restored to its original text",
                cargo_path.display()
            ))),
            Err(restore) => Err(error.context(format!(
                "restoring {} also failed: {restore:#}",
                cargo_path.display()
            ))),
        };
    }
    Ok(())
}

/// Writes `text` to a temporary file next to `path`, then renames it over `path`.
fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let tmp = temp_path_for(path);
    let result = fs::write(&tmp, text).and_then(|()| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.with_context(|| format!("writing {}", path.display()))
}

/// The temporary file used by `write_atomic`: `.<name>.tmp` in the same folder.
fn temp_path_for(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".{name}.tmp"))
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
