//! Update manifest parsing and the version decision, without any I/O.

mod available;
mod decide;
mod error;
mod manifest;
mod service;
#[cfg(test)]
mod service_fakes;
#[cfg(test)]
mod service_retry_tests;
#[cfg(test)]
mod service_tests;
mod staged;
mod state;
mod verify_error;

pub use available::AvailableUpdate;
pub use decide::{decide, UpdateDecision};
pub use error::UpdateError;
pub use manifest::{parse_manifest, Asset, Manifest, ManifestError, MAX_ASSET_SIZE_BYTES};
pub use service::UpdateService;
pub use staged::StagedFile;
pub use state::UpdateState;
pub use verify_error::VerifyError;
