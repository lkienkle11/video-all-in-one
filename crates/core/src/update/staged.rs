//! A downloaded update file waiting in the work folder.

use std::path::PathBuf;

/// A downloaded artifact. The backend computes the SHA-256 while it downloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFile {
    pub path: PathBuf,
    pub size: u64,
    pub sha256: [u8; 32],
}
