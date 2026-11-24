//! The version decision: whether a manifest offers an update to the running app.

use semver::Version;

use super::manifest::{Asset, Manifest};

/// What the updater should do with a valid manifest.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateDecision {
    /// The manifest is equal to or lower than the running version.
    UpToDate,
    /// A newer version exists for this platform.
    Available {
        version: Version,
        size: u64,
        notes_en: String,
        notes_vi: String,
        asset: Asset,
    },
    /// A newer version exists, but it has no artifact for this platform.
    NotForThisPlatform,
}

/// Decides what to offer. An equal or lower version is never offered.
pub fn decide(current: &Version, manifest: &Manifest, platform_key: &str) -> UpdateDecision {
    if !manifest.version.pre.is_empty() {
        return UpdateDecision::UpToDate;
    }
    if manifest.version <= *current {
        return UpdateDecision::UpToDate;
    }
    match manifest.assets.get(platform_key) {
        None => UpdateDecision::NotForThisPlatform,
        Some(asset) => UpdateDecision::Available {
            version: manifest.version.clone(),
            size: asset.size,
            notes_en: manifest.notes_en.clone(),
            notes_vi: manifest.notes_vi.clone(),
            asset: asset.clone(),
        },
    }
}
