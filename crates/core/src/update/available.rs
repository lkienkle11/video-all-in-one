//! An update the manifest offers for this platform.

use semver::Version;

use super::manifest::Asset;

/// A newer version with its artifact, ready to be downloaded.
#[derive(Debug, Clone, PartialEq)]
pub struct AvailableUpdate {
    pub version: Version,
    pub size: u64,
    pub notes_en: String,
    pub notes_vi: String,
    pub asset: Asset,
}
