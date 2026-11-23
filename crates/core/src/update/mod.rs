//! Update manifest parsing and the version decision, without any I/O.

mod decide;
mod manifest;

pub use decide::{decide, UpdateDecision};
pub use manifest::{parse_manifest, Asset, Manifest, ManifestError};
