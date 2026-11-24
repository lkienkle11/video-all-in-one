//! Why an update step failed. Every failure leaves the installed app usable.

use std::fmt;

use super::manifest::ManifestError;
use super::verify::VerifyError;

/// A failed update step.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateError {
    /// The manifest or the artifact could not be fetched.
    Network(String),
    /// The manifest was refused.
    Manifest(ManifestError),
    /// The manifest has no artifact for this platform.
    NoAssetForPlatform,
    /// The downloaded file has a different size from the manifest.
    SizeMismatch { expected: u64, actual: u64 },
    /// The downloaded file failed verification.
    Verify(VerifyError),
    /// Installing the verified file failed.
    Install(String),
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(reason) => write!(f, "the update could not be fetched: {reason}"),
            Self::Manifest(error) => write!(f, "{error}"),
            Self::NoAssetForPlatform => {
                write!(f, "no update is published for this platform")
            }
            Self::SizeMismatch { expected, actual } => write!(
                f,
                "the update file is {actual} bytes but the manifest says {expected}"
            ),
            Self::Verify(error) => write!(f, "{error}"),
            Self::Install(reason) => write!(f, "the update could not be installed: {reason}"),
        }
    }
}

impl std::error::Error for UpdateError {}
