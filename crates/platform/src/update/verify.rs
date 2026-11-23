//! Artifact verification: the SHA-256 digest must match the manifest, and an Ed25519
//! signature over that digest must verify with the compiled-in public key.

use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};

use vaio_core::ports::ArtifactVerifier;
pub use vaio_core::update::VerifyError;

/// The production verifier: the digest check and the Ed25519 signature check.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ed25519Verifier;

impl ArtifactVerifier for Ed25519Verifier {
    fn verify(
        &self,
        digest: [u8; 32],
        expected_sha256_hex: &str,
        signature_b64: &str,
        public_key: &[u8; 32],
    ) -> Result<(), VerifyError> {
        verify_artifact(digest, expected_sha256_hex, signature_b64, public_key)
    }
}

/// Checks the digest first, then the signature over the digest. Nothing is trusted
/// until both pass.
pub fn verify_artifact(
    bytes_digest: [u8; 32],
    expected_sha256_hex: &str,
    signature_b64: &str,
    public_key: &[u8; 32],
) -> Result<(), VerifyError> {
    let actual: String = bytes_digest
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
    key.verify_strict(&bytes_digest, &signature)
        .map_err(|_| VerifyError::BadSignature)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
    use sha2::{Digest, Sha256};

    const SECRET: [u8; 32] = [7; 32];
    const OTHER_SECRET: [u8; 32] = [9; 32];
    const ARTIFACT: &[u8] = b"video-all-in-one 0.1.1 artifact bytes";

    fn digest(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn sign_b64(secret: &[u8; 32], message: &[u8; 32]) -> String {
        let signature = SigningKey::from_bytes(secret).sign(message);
        base64::engine::general_purpose::STANDARD.encode(signature.to_bytes())
    }

    fn public_key(secret: &[u8; 32]) -> [u8; 32] {
        SigningKey::from_bytes(secret).verifying_key().to_bytes()
    }

    #[test]
    fn valid_signature_is_accepted() {
        let d = digest(ARTIFACT);
        let result = verify_artifact(d, &hex(&d), &sign_b64(&SECRET, &d), &public_key(&SECRET));
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn tampered_digest_is_rejected() {
        let original = digest(ARTIFACT);
        let tampered = digest(b"video-all-in-one 0.1.1 artifact, altered");
        let signature = sign_b64(&SECRET, &original);
        let result = verify_artifact(tampered, &hex(&original), &signature, &public_key(&SECRET));
        assert_eq!(result, Err(VerifyError::DigestMismatch));
    }

    #[test]
    fn wrong_key_signature_is_rejected() {
        let d = digest(ARTIFACT);
        let signature = sign_b64(&SECRET, &d);
        let result = verify_artifact(d, &hex(&d), &signature, &public_key(&OTHER_SECRET));
        assert_eq!(result, Err(VerifyError::BadSignature));
    }

    #[test]
    fn truncated_signature_is_malformed() {
        let d = digest(ARTIFACT);
        let signature = sign_b64(&SECRET, &d);
        let truncated = &signature[..signature.len() - 4];
        let result = verify_artifact(d, &hex(&d), truncated, &public_key(&SECRET));
        assert_eq!(result, Err(VerifyError::Malformed));
    }

    #[test]
    fn uppercase_expected_hash_is_a_mismatch() {
        let d = digest(ARTIFACT);
        let signature = sign_b64(&SECRET, &d);
        let result = verify_artifact(d, &hex(&d).to_uppercase(), &signature, &public_key(&SECRET));
        assert_eq!(result, Err(VerifyError::DigestMismatch));
    }

    #[test]
    fn signature_over_other_digest_is_bad_signature() {
        let d = digest(ARTIFACT);
        let signature = sign_b64(&SECRET, &digest(b"some other artifact"));
        let result = verify_artifact(d, &hex(&d), &signature, &public_key(&SECRET));
        assert_eq!(result, Err(VerifyError::BadSignature));
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
        let d = digest(ARTIFACT);
        let signature = sign_b64(&SECRET, &d);
        let result = verify_artifact(d, &hex(&d), &signature, &not_a_point);
        assert_eq!(result, Err(VerifyError::Malformed));
    }

    #[test]
    fn signature_with_one_flipped_bit_is_bad_signature() {
        let d = digest(ARTIFACT);
        let mut signature = SigningKey::from_bytes(&SECRET).sign(&d).to_bytes();
        signature[0] ^= 0x01;
        let encoded = base64::engine::general_purpose::STANDARD.encode(signature);
        let result = verify_artifact(d, &hex(&d), &encoded, &public_key(&SECRET));
        assert_eq!(result, Err(VerifyError::BadSignature));
    }

    #[test]
    fn ed25519_verifier_accepts_valid_and_rejects_tampered() {
        let verifier = Ed25519Verifier;
        let d = digest(ARTIFACT);
        let valid = verifier.verify(d, &hex(&d), &sign_b64(&SECRET, &d), &public_key(&SECRET));
        assert_eq!(valid, Ok(()));
        let tampered = digest(b"video-all-in-one 0.1.1 artifact, altered");
        let rejected = verifier.verify(
            tampered,
            &hex(&tampered),
            &sign_b64(&SECRET, &d),
            &public_key(&SECRET),
        );
        assert_eq!(rejected, Err(VerifyError::BadSignature));
    }

    #[test]
    fn all_zero_public_key_is_rejected() {
        let d = digest(ARTIFACT);
        let signature = sign_b64(&SECRET, &d);
        let result = verify_artifact(d, &hex(&d), &signature, &[0u8; 32]);
        assert_eq!(result, Err(VerifyError::BadSignature));
    }
}
