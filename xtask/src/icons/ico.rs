//! The Windows `.ico` container with PNG entries.

use anyhow::{bail, Result};

/// An `.ico` holding each `(size, png)` entry. A size of 256 is written as 0, as the format asks.
pub(super) fn ico_container(entries: &[(u32, Vec<u8>)]) -> Result<Vec<u8>> {
    if entries.is_empty() || entries.iter().any(|(size, _)| *size == 0 || *size > 256) {
        bail!("an .ico needs at least one entry, each of 1 to 256 pixels");
    }
    let header = 6 + 16 * entries.len();
    let mut out =
        Vec::with_capacity(header + entries.iter().map(|(_, png)| png.len()).sum::<usize>());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&u16::try_from(entries.len())?.to_le_bytes());
    let mut offset = header;
    for (size, png) in entries {
        let side = if *size == 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[side, side, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&u32::try_from(png.len())?.to_le_bytes());
        out.extend_from_slice(&u32::try_from(offset)?.to_le_bytes());
        offset += png.len();
    }
    for (_, png) in entries {
        out.extend_from_slice(png);
    }
    Ok(out)
}
