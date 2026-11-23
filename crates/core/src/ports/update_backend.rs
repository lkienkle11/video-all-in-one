//! The ports the updater needs from the outside world: fetching the manifest, downloading
//! and installing an artifact, and verifying its signature. Adapters live in other crates.

use crate::update::{Asset, StagedFile, UpdateError, VerifyError};

/// Network, file system and installation work for the updater.
pub trait UpdateBackend {
    /// Returns the raw text of the update manifest.
    fn fetch_manifest(&self) -> Result<String, UpdateError>;

    /// Downloads the artifact and reports `(done, total)` while it runs.
    fn download(
        &self,
        asset: &Asset,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<StagedFile, UpdateError>;

    /// Installs a verified, staged artifact.
    fn install(&self, staged: StagedFile) -> Result<(), UpdateError>;
}

/// Checks that a downloaded artifact is the one the manifest signed.
pub trait ArtifactVerifier {
    /// `digest` is the SHA-256 of the downloaded bytes; the signature is over that digest.
    fn verify(
        &self,
        digest: [u8; 32],
        expected_sha256_hex: &str,
        signature_b64: &str,
        public_key: &[u8; 32],
    ) -> Result<(), VerifyError>;
}
