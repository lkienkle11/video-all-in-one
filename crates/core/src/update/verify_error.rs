//! Why a downloaded update artifact must not be installed.

use std::fmt;

/// Why an update artifact must not be installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyError {
    /// The downloaded bytes do not hash to the digest the manifest promised.
    DigestMismatch,
    /// The signature does not verify over the digest with this public key.
    BadSignature,
    /// The signature or the public key is not well formed.
    Malformed,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DigestMismatch => write!(f, "the update file does not match its checksum"),
            Self::BadSignature => write!(f, "the update file signature is not valid"),
            Self::Malformed => write!(f, "the update signature or key is malformed"),
        }
    }
}

impl std::error::Error for VerifyError {}
