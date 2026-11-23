//! The updater state machine. Verification happens inside `download`, before `Ready`.

use semver::Version;

use crate::ports::{ArtifactVerifier, UpdateBackend};

use super::available::AvailableUpdate;
use super::decide::{decide, UpdateDecision};
use super::error::UpdateError;
use super::manifest::parse_manifest;
use super::state::UpdateState;

/// Drives one update from check to install. A disabled service (development channel)
/// never touches the backend or the verifier.
pub struct UpdateService<B: UpdateBackend, V: ArtifactVerifier> {
    backend: B,
    verifier: V,
    current: Version,
    platform_key: String,
    public_key: [u8; 32],
    enabled: bool,
    pub(super) state: UpdateState,
}

impl<B: UpdateBackend, V: ArtifactVerifier> UpdateService<B, V> {
    pub fn new(
        backend: B,
        verifier: V,
        current: Version,
        platform_key: String,
        public_key: [u8; 32],
        enabled: bool,
    ) -> Self {
        Self {
            backend,
            verifier,
            current,
            platform_key,
            public_key,
            enabled,
            state: UpdateState::Idle,
        }
    }

    pub fn state(&self) -> &UpdateState {
        &self.state
    }

    /// Fetches and decides. Allowed from any state except `Downloading`.
    pub fn check(&mut self) {
        if !self.enabled || matches!(self.state, UpdateState::Downloading { .. }) {
            return;
        }
        self.state = UpdateState::Checking;
        let json = match self.backend.fetch_manifest() {
            Ok(json) => json,
            Err(error) => {
                self.state = UpdateState::Failed(error);
                return;
            }
        };
        let manifest = match parse_manifest(&json) {
            Ok(manifest) => manifest,
            Err(error) => {
                self.state = UpdateState::Failed(UpdateError::Manifest(error));
                return;
            }
        };
        self.state = match decide(&self.current, &manifest, &self.platform_key) {
            UpdateDecision::UpToDate => UpdateState::UpToDate,
            UpdateDecision::NotForThisPlatform => {
                UpdateState::Failed(UpdateError::NoAssetForPlatform)
            }
            UpdateDecision::Available {
                version,
                size,
                notes_en,
                notes_vi,
                asset,
            } => UpdateState::Available(AvailableUpdate {
                version,
                size,
                notes_en,
                notes_vi,
                asset,
            }),
        };
    }

    /// Downloads and verifies the offered update. Only allowed from `Available`.
    pub fn download(&mut self) {
        self.download_with_progress(&mut |_, _| {});
    }

    /// Like `download`, and forwards every `(done, total)` report to `observer`.
    pub fn download_with_progress(&mut self, observer: &mut dyn FnMut(u64, Option<u64>)) {
        if !self.enabled {
            return;
        }
        let asset = match &self.state {
            UpdateState::Available(update) => update.asset.clone(),
            _ => return,
        };
        self.state = UpdateState::Downloading {
            done: 0,
            total: Some(asset.size),
        };
        let result = {
            let state = &mut self.state;
            let mut forward = |done: u64, total: Option<u64>| {
                *state = UpdateState::Downloading { done, total };
                observer(done, total);
            };
            self.backend.download(&asset, &mut forward)
        };
        self.state = match result {
            Err(error) => UpdateState::Failed(error),
            Ok(staged) if staged.size != asset.size => {
                UpdateState::Failed(UpdateError::SizeMismatch {
                    expected: asset.size,
                    actual: staged.size,
                })
            }
            Ok(staged) => match self.verifier.verify(
                staged.sha256,
                &asset.sha256,
                &asset.signature,
                &self.public_key,
            ) {
                Ok(()) => UpdateState::Ready(staged),
                Err(error) => UpdateState::Failed(UpdateError::Verify(error)),
            },
        };
    }

    /// Installs a verified file. Only allowed from `Ready`.
    pub fn install(&mut self) {
        if !self.enabled {
            return;
        }
        let staged = match &self.state {
            UpdateState::Ready(staged) => staged.clone(),
            _ => return,
        };
        self.state = match self.backend.install(staged) {
            Ok(()) => UpdateState::Installed,
            Err(error) => UpdateState::Failed(error),
        };
    }
}
