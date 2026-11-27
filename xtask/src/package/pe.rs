//! Reads the subsystem field of a Windows PE executable (2 = GUI, 3 = console).

use anyhow::{bail, Result};

/// The subsystem of the PE image in `bytes`: the optional-header field at offset 68.
pub(crate) fn pe_subsystem(bytes: &[u8]) -> Result<u16> {
    if bytes.get(0..2) != Some(b"MZ") {
        bail!("not a PE file: missing MZ signature");
    }
    let lfanew = bytes
        .get(0x3C..0x40)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .ok_or_else(|| anyhow::anyhow!("not a PE file: DOS header is truncated"))?;
    if bytes.get(lfanew..lfanew + 4) != Some(b"PE\0\0") {
        bail!("not a PE file: missing PE signature");
    }
    // The optional header follows the 20-byte COFF header.
    let field = lfanew + 4 + 20 + 68;
    let subsystem = bytes
        .get(field..field + 2)
        .ok_or_else(|| anyhow::anyhow!("not a PE file: optional header is truncated"))?;
    Ok(u16::from_le_bytes([subsystem[0], subsystem[1]]))
}

#[cfg(test)]
mod tests {
    use super::pe_subsystem;

    /// A minimal PE file: DOS header pointing at "PE\0\0", the 20-byte COFF header, and an
    /// optional header whose subsystem field (offset 68) is `subsystem`.
    fn pe_with_subsystem(subsystem: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 0x200];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        let optional_header = 0x84 + 20;
        bytes[optional_header + 68..optional_header + 70].copy_from_slice(&subsystem.to_le_bytes());
        bytes
    }

    #[test]
    fn gui_pe_header_returns_2() {
        assert_eq!(pe_subsystem(&pe_with_subsystem(2)).unwrap(), 2);
    }

    #[test]
    fn console_pe_header_returns_3() {
        assert_eq!(pe_subsystem(&pe_with_subsystem(3)).unwrap(), 3);
    }

    #[test]
    fn non_pe_bytes_are_an_error() {
        assert!(pe_subsystem(b"not a windows executable").is_err());
    }
}
