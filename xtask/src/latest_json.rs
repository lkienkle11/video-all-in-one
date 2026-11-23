use crate::changelog::{read_notes, Notes};
use crate::set_version::plain_version;
use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use vaio_core::update::parse_manifest;

/// The platform keys of the update artifacts and the file extension each one ships with.
const PLATFORMS: [(&str, &str); 4] = [
    ("macos-aarch64", "zip"),
    ("macos-x86_64", "zip"),
    ("windows-x86_64", "exe"),
    ("linux-x86_64", "deb"),
];
const FILE_PREFIX: &str = "video-all-in-one-";

/// Reads the notes of `version` from `changelog`, builds the schema 1 manifest from the
/// artifacts in `assets_dir` and writes it to `out` (default `<assets_dir>/latest.json`).
/// Nothing is written when any step fails.
pub fn run(
    version: &str,
    changelog: &Path,
    assets_dir: &Path,
    base_url: &str,
    out: Option<&Path>,
) -> Result<()> {
    let text = fs::read_to_string(changelog)
        .with_context(|| format!("reading {}", changelog.display()))?;
    let notes = read_notes(&text, version)?;
    let manifest = build_manifest(version, &notes, assets_dir, base_url)?;
    let count = parse_manifest(&manifest)?.assets.len();
    let path = match out {
        Some(path) => path.to_path_buf(),
        None => assets_dir.join("latest.json"),
    };
    fs::write(&path, manifest).with_context(|| format!("writing {}", path.display()))?;
    println!("wrote {} with {count} assets", path.display());
    Ok(())
}

/// Builds the manifest text (pretty JSON) and checks it with the app's own parser.
pub fn build_manifest(
    version: &str,
    notes: &Notes,
    assets_dir: &Path,
    base_url: &str,
) -> Result<String> {
    plain_version(version)?;
    if !base_url.starts_with("https://") {
        bail!("base URL `{base_url}` must start with https://");
    }
    let base = if base_url.ends_with('/') {
        base_url.to_string()
    } else {
        format!("{base_url}/")
    };
    let artifacts = find_artifacts(assets_dir, version)?;
    let mut assets = BTreeMap::new();
    for (platform, name) in &artifacts {
        let url = format!("{base}{name}");
        assets.insert(platform.clone(), asset(assets_dir, name, &url)?);
    }
    let value = serde_json::json!({
        "schema": 1,
        "version": version,
        "notes_en": notes.english,
        "notes_vi": notes.vietnamese,
        "assets": assets,
    });
    let text = serde_json::to_string_pretty(&value)?;
    let manifest = parse_manifest(&text)
        .map_err(|error| anyhow!("generated manifest was refused: {error}"))?;
    if manifest.version.to_string() != version || manifest.assets.len() != artifacts.len() {
        bail!("generated manifest does not match version {version} and its artifacts");
    }
    Ok(text)
}

/// Records one artifact per platform key; a second artifact for the same key is an error.
pub(crate) fn add_unique(
    artifacts: &mut BTreeMap<String, String>,
    platform: &str,
    name: &str,
) -> Result<()> {
    if let Some(existing) = artifacts.get(platform) {
        bail!("platform {platform} has two artifacts: {existing} and {name}");
    }
    artifacts.insert(platform.to_string(), name.to_string());
    Ok(())
}

/// The artifacts of `version` in `assets_dir`, keyed by platform key. Files that match the
/// artifact name pattern with another version are errors; all other files are ignored.
fn find_artifacts(assets_dir: &Path, version: &str) -> Result<BTreeMap<String, String>> {
    let mut artifacts = BTreeMap::new();
    let entries =
        fs::read_dir(assets_dir).with_context(|| format!("reading {}", assets_dir.display()))?;
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let Some((platform, found)) = match_artifact(name) else {
            continue;
        };
        if found != version {
            bail!("artifact {name} is for version {found}, not {version}");
        }
        add_unique(&mut artifacts, platform, name)?;
    }
    if artifacts.is_empty() {
        bail!(
            "no update artifacts for version {version} in {}",
            assets_dir.display()
        );
    }
    Ok(artifacts)
}

/// The platform key and version of a `video-all-in-one-<version>-<platform>.<ext>` name.
fn match_artifact(name: &str) -> Option<(&'static str, &str)> {
    let rest = name.strip_prefix(FILE_PREFIX)?;
    for &(platform, ext) in PLATFORMS.iter() {
        let suffix = format!("-{platform}.{ext}");
        if let Some(version) = rest.strip_suffix(&suffix) {
            if !version.is_empty() {
                return Some((platform, version));
            }
        }
    }
    None
}

/// The manifest entry of one artifact: size, SHA-256 and the content of its `.sig` file.
fn asset(assets_dir: &Path, name: &str, url: &str) -> Result<serde_json::Value> {
    let path = assets_dir.join(name);
    let size = fs::metadata(&path)
        .with_context(|| format!("reading {}", path.display()))?
        .len();
    if size == 0 {
        bail!("artifact {name} is empty");
    }
    let sha256 = sha256_hex(&path)?;
    let sig_path = assets_dir.join(format!("{name}.sig"));
    let raw = fs::read_to_string(&sig_path)
        .map_err(|_| anyhow!("artifact {name} has no signature file {name}.sig"))?;
    let signature = raw.trim();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(signature)
        .map_err(|_| anyhow!("signature of {name} is not base64"))?;
    if bytes.len() != 64 {
        bail!(
            "signature of {name} decodes to {} bytes, expected 64",
            bytes.len()
        );
    }
    Ok(serde_json::json!({
        "url": url,
        "size": size,
        "sha256": sha256,
        "signature": signature,
    }))
}

/// Lowercase hex SHA-256 of a file, read in 8 KiB chunks.
fn sha256_hex(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("reading {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
