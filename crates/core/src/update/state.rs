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
    Downloading { done: u64, total: Option<u64> },
    Ready(StagedFile),
    Installed,
    Failed(UpdateError),
}
