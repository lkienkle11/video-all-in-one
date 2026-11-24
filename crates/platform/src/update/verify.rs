//! Artifact verification: the SHA-256 digest must match the manifest, and an Ed25519
//! signature over the claim (version, platform, size and digest) must verify with the
//! compiled-in public key.

use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};

use vaio_core::ports::ArtifactVerifier;
use vaio_core::update::UpdateClaim;
pub use vaio_core::update::VerifyError;

/// The production verifier: the digest check and the Ed25519 signature check.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ed25519Verifier;

impl ArtifactVerifier for Ed25519Verifier {
    fn verify(
        &self,
        claim: &UpdateClaim,
        expected_sha256_hex: &str,
        signature_b64: &str,
        public_key: &[u8; 32],
    ) -> Result<(), VerifyError> {
        verify_artifact(claim, expected_sha256_hex, signature_b64, public_key)
    }
}

/// Checks the digest first, then the signature over `claim.signed_message()`. Nothing is
/// trusted until both pass.
pub fn verify_artifact(
    claim: &UpdateClaim,
    expected_sha256_hex: &str,
    signature_b64: &str,
    public_key: &[u8; 32],
) -> Result<(), VerifyError> {
    let actual: String = claim
        .sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual != expected_sha256_hex {
        return Err(VerifyError::DigestMismatch);
    }
    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .map_err(|_| VerifyError::Malformed)?;
    let signature_array: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| VerifyError::Malformed)?;
    let key = VerifyingKey::from_bytes(public_key).map_err(|_| VerifyError::Malformed)?;
    let signature = Signature::from_bytes(&signature_array);
    key.verify_strict(&claim.signed_message(), &signature)
        .map_err(|_| VerifyError::BadSignature)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
    use sha2::{Digest, Sha256};
    use vaio_core::update::signed_message;

    const SECRET: [u8; 32] = [7; 32];
    const OTHER_SECRET: [u8; 32] = [9; 32];
    const ARTIFACT: &[u8] = b"video-all-in-one 0.1.1 artifact bytes";

    fn digest(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn claim_of(d: [u8; 32]) -> UpdateClaim {
        UpdateClaim {
            version: "0.1.1".to_string(),
            platform_key: "linux-x86_64".to_string(),
            size: 4096,
            sha256: d,
        }
    }

    fn sign_claim(secret: &[u8; 32], claim: &UpdateClaim) -> String {
        let signature = SigningKey::from_bytes(secret).sign(&claim.signed_message());
        base64::engine::general_purpose::STANDARD.encode(signature.to_bytes())
    }

    fn public_key(secret: &[u8; 32]) -> [u8; 32] {
        SigningKey::from_bytes(secret).verifying_key().to_bytes()
    }

    fn check(claim: &UpdateClaim, signature: &str) -> Result<(), VerifyError> {
        verify_artifact(claim, &hex(&claim.sha256), signature, &public_key(&SECRET))
    }

    #[test]
    fn valid_claim_is_accepted() {
        let claim = claim_of(digest(ARTIFACT));
        assert_eq!(check(&claim, &sign_claim(&SECRET, &claim)), Ok(()));
    }

    #[test]
    fn signature_for_version_a_is_rejected_for_version_b() {
        let signed = claim_of(digest(ARTIFACT));
        let signature = sign_claim(&SECRET, &signed);
        let offered = UpdateClaim {
            version: "99.0.0".to_string(),
            ..signed
        };
        assert_eq!(check(&offered, &signature), Err(VerifyError::BadSignature));
    }

    #[test]
    fn signature_for_platform_x_is_rejected_for_platform_y() {
        let signed = claim_of(digest(ARTIFACT));
        let signature = sign_claim(&SECRET, &signed);
        let offered = UpdateClaim {
            platform_key: "windows-x86_64".to_string(),
            ..signed
        };
        assert_eq!(check(&offered, &signature), Err(VerifyError::BadSignature));
    }

    #[test]
    fn signature_for_size_n_is_rejected_for_size_m() {
        let signed = claim_of(digest(ARTIFACT));
        let signature = sign_claim(&SECRET, &signed);
        let offered = UpdateClaim {
            size: signed.size + 1,
            ..signed
        };
        assert_eq!(check(&offered, &signature), Err(VerifyError::BadSignature));
    }

    #[test]
    fn signature_over_the_bare_digest_is_rejected() {
        let claim = claim_of(digest(ARTIFACT));
        let signature = SigningKey::from_bytes(&SECRET).sign(&claim.sha256);
        let encoded = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());
        assert_eq!(check(&claim, &encoded), Err(VerifyError::BadSignature));
    }

    #[test]
    fn signature_over_the_documented_message_is_accepted() {
        let claim = claim_of(digest(ARTIFACT));
        let message = signed_message("0.1.1", "linux-x86_64", 4096, &claim.sha256);
        let signature = SigningKey::from_bytes(&SECRET).sign(&message);
        let encoded = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());
        assert_eq!(check(&claim, &encoded), Ok(()));
    }

    #[test]
    fn tampered_digest_is_rejected() {
        let original = claim_of(digest(ARTIFACT));
        let tampered = claim_of(digest(b"video-all-in-one 0.1.1 artifact, altered"));
        let signature = sign_claim(&SECRET, &original);
        let result = verify_artifact(
            &tampered,
            &hex(&original.sha256),
            &signature,
            &public_key(&SECRET),
        );
        assert_eq!(result, Err(VerifyError::DigestMismatch));
    }

    #[test]
    fn wrong_key_signature_is_rejected() {
        let claim = claim_of(digest(ARTIFACT));
        let signature = sign_claim(&SECRET, &claim);
        let result = verify_artifact(
            &claim,
            &hex(&claim.sha256),
            &signature,
            &public_key(&OTHER_SECRET),
        );
        assert_eq!(result, Err(VerifyError::BadSignature));
    }

    #[test]
    fn truncated_signature_is_malformed() {
        let claim = claim_of(digest(ARTIFACT));
        let signature = sign_claim(&SECRET, &claim);
        let truncated = &signature[..signature.len() - 4];
        assert_eq!(check(&claim, truncated), Err(VerifyError::Malformed));
    }

    #[test]
    fn uppercase_expected_hash_is_a_mismatch() {
        let claim = claim_of(digest(ARTIFACT));
        let signature = sign_claim(&SECRET, &claim);
        let result = verify_artifact(
            &claim,
            &hex(&claim.sha256).to_uppercase(),
            &signature,
            &public_key(&SECRET),
        );
        assert_eq!(result, Err(VerifyError::DigestMismatch));
    }

    #[test]
    fn signature_over_other_digest_is_bad_signature() {
        let claim = claim_of(digest(ARTIFACT));
        let signature = sign_claim(&SECRET, &claim_of(digest(b"some other artifact")));
        assert_eq!(check(&claim, &signature), Err(VerifyError::BadSignature));
    }

    #[test]
    fn invalid_public_key_is_malformed() {
        let not_a_point = (0u8..=255)
            .map(|first| {
                let mut key = [0u8; 32];
                key[0] = first;
                key
            })
            .find(|key| VerifyingKey::from_bytes(key).is_err())
            .expect("some 32-byte string is not a curve point");
        let claim = claim_of(digest(ARTIFACT));
        let signature = sign_claim(&SECRET, &claim);
        let result = verify_artifact(&claim, &hex(&claim.sha256), &signature, &not_a_point);
        assert_eq!(result, Err(VerifyError::Malformed));
    }

    #[test]
    fn signature_with_one_flipped_bit_is_bad_signature() {
        let claim = claim_of(digest(ARTIFACT));
        let mut signature = SigningKey::from_bytes(&SECRET)
            .sign(&claim.signed_message())
            .to_bytes();
        signature[0] ^= 0x01;
        let encoded = base64::engine::general_purpose::STANDARD.encode(signature);
        assert_eq!(check(&claim, &encoded), Err(VerifyError::BadSignature));
    }

    #[test]
    fn ed25519_verifier_accepts_valid_and_rejects_tampered() {
        let verifier = Ed25519Verifier;
        let claim = claim_of(digest(ARTIFACT));
        let signature = sign_claim(&SECRET, &claim);
        let valid = verifier.verify(
            &claim,
            &hex(&claim.sha256),
            &signature,
            &public_key(&SECRET),
        );
        assert_eq!(valid, Ok(()));
        let tampered = claim_of(digest(b"video-all-in-one 0.1.1 artifact, altered"));
        let rejected = verifier.verify(
            &tampered,
            &hex(&tampered.sha256),
            &signature,
            &public_key(&SECRET),
        );
        assert_eq!(rejected, Err(VerifyError::BadSignature));
    }

    #[test]
    fn all_zero_public_key_is_rejected() {
        let claim = claim_of(digest(ARTIFACT));
        let signature = sign_claim(&SECRET, &claim);
        let result = verify_artifact(&claim, &hex(&claim.sha256), &signature, &[0u8; 32]);
        assert_eq!(result, Err(VerifyError::BadSignature));
    }
}
