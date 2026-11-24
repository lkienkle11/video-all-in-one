//! The macOS `.icns` container with PNG entries.

use anyhow::{anyhow, Result};

/// The PNG based type codes of the format and the pixel size each one holds.
pub(super) const TYPES: [(&str, u32); 11] = [
    ("icp4", 16),
    ("icp5", 32),
    ("icp6", 64),
    ("ic07", 128),
    ("ic08", 256),
    ("ic09", 512),
    ("ic10", 1024),
    ("ic11", 32),
    ("ic12", 64),
    ("ic13", 256),
    ("ic14", 512),
];

/// An `.icns` with one entry per type code; `pngs` holds one `(size, png)` per pixel size.
/// Every number in the format is big endian and counts the 8 byte header of its block.
pub(super) fn icns_container(pngs: &[(u32, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    for (code, size) in TYPES {
        let (_, png) = pngs
            .iter()
            .find(|(s, _)| *s == size)
            .ok_or_else(|| anyhow!("no {size} pixel PNG for the type {code}"))?;
        body.extend_from_slice(code.as_bytes());
        body.extend_from_slice(&u32::try_from(8 + png.len())?.to_be_bytes());
        body.extend_from_slice(png);
    }
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&u32::try_from(8 + body.len())?.to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}
