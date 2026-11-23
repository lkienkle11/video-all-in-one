//! The update manifest (`latest.json`, schema 1): parsing and validation.

use std::collections::BTreeMap;
use std::fmt;

use semver::Version;
use serde::Deserialize;

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
    if !is_lower_hex_sha256(&asset.sha256) {
        return Err(invalid(
            platform,
            "sha256 is not 64 lowercase hex characters",
        ));
    }
    if !is_canonical_base64_of_len(&asset.signature, 64) {
        return Err(invalid(platform, "signature is not base64 of 64 bytes"));
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

/// Checks standard padded base64 that decodes to exactly `byte_len` bytes and has
/// zero unused bits, so only the one canonical spelling is accepted. Core has no
/// base64 dependency; the platform crate performs the full decode when it verifies.
fn is_canonical_base64_of_len(text: &str, byte_len: usize) -> bool {
    let remainder = byte_len % 3;
    let padding = match remainder {
        0 => 0,
        1 => 2,
        _ => 1,
    };
    let encoded_len = byte_len.div_ceil(3) * 4;
    let bytes = text.as_bytes();
    if bytes.len() != encoded_len {
        return false;
    }
    let (body, pad) = bytes.split_at(encoded_len - padding);
    if !pad.iter().all(|&byte| byte == b'=') || !body.iter().all(|&b| base64_value(b).is_some()) {
        return false;
    }
    match (remainder, body.last()) {
        (0, _) => true,
        (1, Some(&last)) => base64_value(last).is_some_and(|value| value & 0x0F == 0),
        (2, Some(&last)) => base64_value(last).is_some_and(|value| value & 0x03 == 0),
        _ => false,
    }
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
