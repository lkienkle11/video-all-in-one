//! The updater's state as the UI sees it.

use super::available::AvailableUpdate;
use super::error::UpdateError;
use super::staged::StagedFile;

/// Where the updater is. Only `Ready` holds a fully verified file.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available(AvailableUpdate),
    Downloading {
        done: u64,
        total: Option<u64>,
    },
    Ready(StagedFile),
    /// An install failed; the verified file is kept so that install can be retried.
    InstallFailed {
        staged: StagedFile,
        error: UpdateError,
    },
    Installed,
    Failed(UpdateError),
}
