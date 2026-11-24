//! What a downloaded update artifact must prove: the claim a signature binds, and why a
//! verification can fail.

use std::fmt;

/// The facts the signature binds. Built by the updater from what it knows, and by the
/// release tooling from the artifact file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateClaim {
    pub version: String,
    pub platform_key: String,
    pub size: u64,
    pub sha256: [u8; 32],
}

impl UpdateClaim {
    /// The exact bytes that are signed for this claim.
    pub fn signed_message(&self) -> Vec<u8> {
        signed_message(&self.version, &self.platform_key, self.size, &self.sha256)
    }
}

/// The signed message: `vaio-update-v1\n<version>\n<platform_key>\n<size decimal>\n<sha256
/// lowercase hex>\n`. The one place that builds it.
pub fn signed_message(version: &str, platform_key: &str, size: u64, sha256: &[u8; 32]) -> Vec<u8> {
    let hex: String = sha256.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("vaio-update-v1\n{version}\n{platform_key}\n{size}\n{hex}\n").into_bytes()
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_has_the_exact_documented_layout() {
        let message = signed_message("1.2.3", "linux-x86_64", 42, &[0xab; 32]);
        let expected = format!(
            "vaio-update-v1\n1.2.3\nlinux-x86_64\n42\n{}\n",
            "ab".repeat(32)
        );
        assert_eq!(message, expected.into_bytes());
    }

    #[test]
    fn claim_message_matches_the_function() {
        let claim = UpdateClaim {
            version: "0.1.0".into(),
            platform_key: "macos-aarch64".into(),
            size: 7,
            sha256: [1; 32],
        };
        assert_eq!(
            claim.signed_message(),
            signed_message("0.1.0", "macos-aarch64", 7, &[1; 32])
        );
    }
}
