use crate::release::changelog::{read_notes, Notes};
use crate::release::set_version::plain_version;
use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use vaio_core::update::{parse_manifest, UpdateClaim};
use vaio_platform::update::verify::verify_artifact;

/// The platform keys of the update artifacts and the file extension each one ships with.
pub(crate) const PLATFORMS: [(&str, &str); 4] = [
    ("macos-aarch64", "zip"),
    ("macos-x86_64", "zip"),
    ("windows-x86_64", "exe"),
    ("linux-x86_64", "deb"),
];
pub(crate) const FILE_PREFIX: &str = "video-all-in-one-";
/// Between the version and the platform key in the name of a development build.
pub(crate) const DEV_MARKER: &str = "-dev-";

/// Reads the notes of `version` from `changelog`, builds the schema 1 manifest from the
/// artifacts in `assets_dir` and writes it to `out` (default `<assets_dir>/latest.json`).
/// Nothing is written when any step fails.
pub fn run(
    version: &str,
    changelog: &Path,
    assets_dir: &Path,
    base_url: &str,
    out: Option<&Path>,
    public_key: &[u8; 32],
    required: &[String],
) -> Result<()> {
    let text = fs::read_to_string(changelog)
        .with_context(|| format!("reading {}", changelog.display()))?;
    let notes = read_notes(&text, version)?;
    let manifest = build_manifest(version, &notes, assets_dir, base_url, public_key, required)?;
    let count = parse_manifest(&manifest)?.assets.len();
    let path = match out {
        Some(path) => path.to_path_buf(),
        None => assets_dir.join("latest.json"),
    };
    fs::write(&path, manifest).with_context(|| format!("writing {}", path.display()))?;
    println!("wrote {} with {count} assets", path.display());
    Ok(())
}

/// Decodes the base64 Ed25519 public key given on the command line.
pub fn decode_public_key(text: &str) -> Result<[u8; 32]> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text.trim())
        .map_err(|_| anyhow!("public key is not base64"))?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("public key must decode to 32 bytes, found {}", bytes.len()))
}

/// Builds the manifest text (pretty JSON) and checks it with the app's own parser.
pub fn build_manifest(
    version: &str,
    notes: &Notes,
    assets_dir: &Path,
    base_url: &str,
    public_key: &[u8; 32],
    required: &[String],
) -> Result<String> {
    plain_version(version)?;
    check_base_url(base_url)?;
    check_required_keys(required)?;
    let base = if base_url.ends_with('/') {
        base_url.to_string()
    } else {
        format!("{base_url}/")
    };
    let artifacts = find_artifacts(assets_dir, version)?;
    check_required_present(required, &artifacts)?;
    let mut assets = BTreeMap::new();
    for (platform, name) in &artifacts {
        let url = format!("{base}{name}");
        assets.insert(
            platform.clone(),
            asset(assets_dir, name, (platform, version), &url, public_key)?,
        );
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
/// A development build (`video-all-in-one-<version>-dev-<platform>.<ext>`) is never an update
/// artifact, so it never matches.
pub(crate) fn match_artifact(name: &str) -> Option<(&'static str, &str)> {
    let rest = name.strip_prefix(FILE_PREFIX)?;
    if rest.contains(DEV_MARKER) {
        return None;
    }
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
/// The signature must verify over the claim (version, platform, size, digest) with `public_key`.
fn asset(
    assets_dir: &Path,
    name: &str,
    (platform, version): (&str, &str),
    url: &str,
    public_key: &[u8; 32],
) -> Result<serde_json::Value> {
    let path = assets_dir.join(name);
    let size = fs::metadata(&path)
        .with_context(|| format!("reading {}", path.display()))?
        .len();
    if size == 0 {
        bail!("artifact {name} is empty");
    }
    let digest = sha256_digest(&path)?;
    let sha256 = to_hex(&digest);
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
    let claim = UpdateClaim {
        version: version.to_string(),
        platform_key: platform.to_string(),
        size,
        sha256: digest,
    };
    verify_artifact(&claim, &sha256, signature, public_key)
        .map_err(|error| anyhow!("artifact {name} failed signature check: {error}"))?;
    Ok(serde_json::json!({
        "url": url,
        "size": size,
        "sha256": sha256,
        "signature": signature,
    }))
}

/// The base URL must be https, with a host and no credentials, query, fragment or whitespace.
fn check_base_url(base_url: &str) -> Result<()> {
    let Some(rest) = base_url.strip_prefix("https://") else {
        bail!("base URL `{base_url}` must start with https://");
    };
    if base_url.contains(['?', '#']) || base_url.chars().any(char::is_whitespace) {
        bail!("base URL `{base_url}` must not contain a query, a fragment or whitespace");
    }
    let host = rest.split('/').next().unwrap_or_default();
    if host.is_empty() {
        bail!("base URL `{base_url}` has no host");
    }
    if host.contains('@') {
        bail!("base URL `{base_url}` must not contain credentials");
    }
    Ok(())
}

/// Every required key must be one of the four platform keys.
fn check_required_keys(required: &[String]) -> Result<()> {
    for key in required {
        if !PLATFORMS.iter().any(|(platform, _)| platform == key) {
            let known: Vec<&str> = PLATFORMS.iter().map(|(platform, _)| *platform).collect();
            bail!(
                "unknown required platform `{key}`; expected one of {}",
                known.join(", ")
            );
        }
    }
    Ok(())
}

/// Every required platform must have an artifact; all missing keys are named, sorted.
fn check_required_present(required: &[String], artifacts: &BTreeMap<String, String>) -> Result<()> {
    let missing: BTreeSet<&str> = required
        .iter()
        .map(String::as_str)
        .filter(|key| !artifacts.contains_key(*key))
        .collect();
    if !missing.is_empty() {
        let names: Vec<&str> = missing.into_iter().collect();
        bail!("required platforms have no artifact: {}", names.join(", "));
    }
    Ok(())
}

/// SHA-256 of a file, read in 8 KiB chunks.
fn sha256_digest(path: &Path) -> Result<[u8; 32]> {
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
    Ok(hasher.finalize().into())
}

/// Lowercase hex of a digest.
fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
