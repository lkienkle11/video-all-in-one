use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const KEY_ENV_VAR: &str = "UPDATE_SIGNING_KEY";

#[derive(Debug)]
pub struct SignOutput {
    pub sha256_hex: String,
    pub signature_b64: String,
    // Read by callers in tests; the subcommand prints only the two stdout lines.
    #[allow(dead_code)]
    pub sig_path: PathBuf,
}

/// Signs an artifact with the key from `UPDATE_SIGNING_KEY` and prints the digest and signature.
pub fn run(file: &Path) -> Result<()> {
    let key = std::env::var(KEY_ENV_VAR)
        .map_err(|_| anyhow!("{KEY_ENV_VAR} is not set or is not valid text"))?;
    let output = sign_file(file, &key)?;
    println!("sha256={}", output.sha256_hex);
    println!("signature={}", output.signature_b64);
    Ok(())
}

/// Hashes the file in 8 KiB chunks, signs the 32-byte digest and writes `<file>.sig`.
/// Errors never include the key text.
pub fn sign_file(path: &Path, key_b64: &str) -> Result<SignOutput> {
    let seed = decode_seed(key_b64)?;
    let digest = hash_file(path)?;
    let signature = SigningKey::from_bytes(&seed).sign(&digest);
    let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());
    let sha256_hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    let sig_path = sig_path_for(path);
    fs::write(&sig_path, format!("{signature_b64}\n"))
        .with_context(|| format!("writing {}", sig_path.display()))?;
    Ok(SignOutput {
        sha256_hex,
        signature_b64,
        sig_path,
    })
}

fn decode_seed(key_b64: &str) -> Result<[u8; 32]> {
    let text = key_b64.trim();
    if text.is_empty() {
        bail!("{KEY_ENV_VAR} is not set or is empty");
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|_| anyhow!("{KEY_ENV_VAR} is not valid base64"))?;
    bytes.as_slice().try_into().map_err(|_| {
        anyhow!(
            "{KEY_ENV_VAR} must decode to 32 bytes, found {}",
            bytes.len()
        )
    })
}

fn hash_file(path: &Path) -> Result<[u8; 32]> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("reading {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

fn sig_path_for(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".sig");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::sign_file;
    use crate::release::set_version::fixture::Fixture;
    use base64::Engine;
    use ed25519_dalek::SigningKey;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::path::{Path, PathBuf};
    use vaio_platform::update::verify::{verify_artifact, VerifyError};

    const SEED: [u8; 32] = [11; 32];

    fn key_b64() -> String {
        base64::engine::general_purpose::STANDARD.encode(SEED)
    }

    fn public_key() -> [u8; 32] {
        SigningKey::from_bytes(&SEED).verifying_key().to_bytes()
    }

    fn digest_of(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }

    fn write_artifact(root: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn sign_then_verify_roundtrip_with_platform_verify() {
        let root = Fixture::new();
        let bytes = b"update artifact bytes";
        let file = write_artifact(&root, "app.bin", bytes);
        let out = sign_file(&file, &key_b64()).unwrap();
        assert_eq!(
            verify_artifact(
                digest_of(bytes),
                &out.sha256_hex,
                &out.signature_b64,
                &public_key()
            ),
            Ok(())
        );

        let mut tampered = bytes.to_vec();
        tampered[0] ^= 0x01;
        fs::write(&file, &tampered).unwrap();
        let rehashed = digest_of(&fs::read(&file).unwrap());
        assert_eq!(
            verify_artifact(rehashed, &out.sha256_hex, &out.signature_b64, &public_key()),
            Err(VerifyError::DigestMismatch)
        );
    }

    #[test]
    fn sign_digest_of_a_large_file_matches_sha2() {
        let root = Fixture::new();
        let bytes: Vec<u8> = (0..3 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
        let file = write_artifact(&root, "large.bin", &bytes);
        let out = sign_file(&file, &key_b64()).unwrap();
        let expected: String = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(out.sha256_hex, expected);
    }

    #[test]
    fn sign_writes_the_sig_file_next_to_the_artifact() {
        let root = Fixture::new();
        let file = write_artifact(&root, "app.bin", b"bytes");
        let out = sign_file(&file, &key_b64()).unwrap();
        assert_eq!(out.sig_path, root.join("app.bin.sig"));
        assert_eq!(
            fs::read_to_string(&out.sig_path).unwrap(),
            format!("{}\n", out.signature_b64)
        );
    }

    #[test]
    fn sign_rejects_a_malformed_key_without_echoing_it() {
        let root = Fixture::new();
        let file = write_artifact(&root, "app.bin", b"bytes");
        for bad in ["!!not-base64-value!!", "", "QUJD"] {
            let error = sign_file(&file, bad).expect_err(bad);
            let text = format!("{error:#}");
            assert!(bad.is_empty() || !text.contains(bad), "{text}");
        }
    }

    #[test]
    fn sign_rejects_a_key_of_the_wrong_length() {
        let root = Fixture::new();
        let file = write_artifact(&root, "app.bin", b"bytes");
        for len in [31usize, 33, 64] {
            let key = base64::engine::general_purpose::STANDARD.encode(vec![7u8; len]);
            assert!(sign_file(&file, &key).is_err(), "accepted {len} bytes");
        }
    }
}
