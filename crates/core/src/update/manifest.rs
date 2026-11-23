//! The update manifest (`latest.json`, schema 1): parsing and validation.

use std::collections::BTreeMap;
use std::fmt;

use base64::Engine;
use semver::Version;
use serde::Deserialize;

/// The largest artifact size the app accepts: 4 GiB, in bytes. A manifest that
/// declares a bigger asset is refused, so a hostile size cannot drive a huge download.
pub const MAX_ASSET_SIZE_BYTES: u64 = 4_294_967_296;

/// A parsed schema 1 update manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub version: Version,
    pub notes_en: String,
    pub notes_vi: String,
    pub assets: BTreeMap<String, Asset>,
}

/// One downloadable artifact for one platform.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Asset {
    pub url: String,
    pub size: u64,
    pub sha256: String,
    pub signature: String,
}

/// Why a manifest was refused. A refused manifest is never partly used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// The manifest declares a schema this build does not know. Nothing else was read.
    UnknownSchema(u32),
    /// The manifest is not valid JSON, or is not valid for its declared schema.
    Invalid(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSchema(schema) => {
                write!(f, "update manifest schema {schema} is not supported")
            }
            Self::Invalid(reason) => write!(f, "update manifest is invalid: {reason}"),
        }
    }
}

impl std::error::Error for ManifestError {}

/// Parses a manifest. Only the `schema` field is read before the schema is known.
pub fn parse_manifest(json: &str) -> Result<Manifest, ManifestError> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|error| ManifestError::Invalid(format!("not JSON: {error}")))?;
    let schema = value
        .get("schema")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            ManifestError::Invalid("schema is missing or not an unsigned integer".to_string())
        })?;
    let schema = u32::try_from(schema)
        .map_err(|_| ManifestError::Invalid("schema is out of range".to_string()))?;
    if schema != 1 {
        return Err(ManifestError::UnknownSchema(schema));
    }
    let manifest: Manifest =
        serde_json::from_value(value).map_err(|error| ManifestError::Invalid(error.to_string()))?;
    for (platform, asset) in &manifest.assets {
        validate_asset(platform, asset)?;
    }
    Ok(manifest)
}

fn validate_asset(platform: &str, asset: &Asset) -> Result<(), ManifestError> {
    if asset.size == 0 {
        return Err(invalid(platform, "size is zero"));
    }
    if asset.size > MAX_ASSET_SIZE_BYTES {
        return Err(invalid(platform, "size is above the 4 GiB limit"));
    }
    if !is_lower_hex_sha256(&asset.sha256) {
        return Err(invalid(
            platform,
            "sha256 is not 64 lowercase hex characters",
        ));
    }
    match base64::engine::general_purpose::STANDARD.decode(&asset.signature) {
        Ok(bytes) if bytes.len() == 64 => {}
        _ => return Err(invalid(platform, "signature is not base64 of 64 bytes")),
    }
    Ok(())
}

fn invalid(platform: &str, reason: &str) -> ManifestError {
    ManifestError::Invalid(format!("asset {platform}: {reason}"))
}

fn is_lower_hex_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
