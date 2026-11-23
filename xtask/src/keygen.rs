use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use ed25519_dalek::SigningKey;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

pub const KEY_FILE_NAME: &str = "update-signing.key";

#[derive(Debug)]
pub struct KeygenOutput {
    pub key_path: PathBuf,
    // Read by callers in tests; the subcommand prints `rust_literal` instead.
    #[allow(dead_code)]
    pub public_key: [u8; 32],
    pub rust_literal: String,
}

/// Generates a key pair with OS randomness and prints the public key.
pub fn run(out_dir: &Path) -> Result<()> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed)
        .map_err(|error| anyhow!("generating key material failed: {error}"))?;
    let output = keygen(out_dir, seed)?;
    print!("{}", output.rust_literal);
    println!(
        "Private key written to {}. Back it up and store it as the GitHub secret UPDATE_SIGNING_KEY.",
        output.key_path.display()
    );
    Ok(())
}

/// Writes the seed to `<out_dir>/update-signing.key` and builds the public key output.
/// An existing key file is never replaced.
pub fn keygen(out_dir: &Path, seed: [u8; 32]) -> Result<KeygenOutput> {
    fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    let key_path = out_dir.join(KEY_FILE_NAME);
    let public_key = SigningKey::from_bytes(&seed).verifying_key().to_bytes();
    let mut file = match create_key_file(&key_path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            bail!(
                "{} already exists; refusing to overwrite it",
                key_path.display()
            )
        }
        Err(error) => {
            return Err(error).with_context(|| format!("creating {}", key_path.display()))
        }
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(seed);
    writeln!(file, "{encoded}").with_context(|| format!("writing {}", key_path.display()))?;
    Ok(KeygenOutput {
        key_path,
        public_key,
        rust_literal: rust_literal(&public_key),
    })
}

/// Creates the key file owner-only at creation time, so it is never readable by others.
fn create_key_file(path: &Path) -> std::io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// The public key as a Rust constant: four rows of eight lowercase hex bytes.
fn rust_literal(public_key: &[u8; 32]) -> String {
    let mut text = String::from("pub const UPDATE_PUBLIC_KEY: [u8; 32] = [\n");
    for row in public_key.chunks(8) {
        let entries: Vec<String> = row.iter().map(|byte| format!("0x{byte:02x}")).collect();
        text.push_str(&format!("    {},\n", entries.join(", ")));
    }
    text.push_str("];\n");
    text
}

#[cfg(test)]
mod tests {
    use super::keygen;
    use crate::set_version::fixture::Fixture;
    use base64::Engine;
    use ed25519_dalek::SigningKey;
    use std::fs;

    const SEED: [u8; 32] = [42; 32];
    const OTHER_SEED: [u8; 32] = [3; 32];

    fn seed_b64(seed: &[u8; 32]) -> String {
        base64::engine::general_purpose::STANDARD.encode(seed)
    }

    #[test]
    #[cfg(unix)]
    fn keygen_writes_private_key_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let root = Fixture::new();
        let out = keygen(&root, SEED).unwrap();
        let mode = fs::metadata(&out.key_path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn keygen_refuses_to_overwrite_an_existing_key() {
        let root = Fixture::new();
        let first = keygen(&root, SEED).unwrap();
        let before = fs::read_to_string(&first.key_path).unwrap();
        assert!(keygen(&root, OTHER_SEED).is_err());
        assert_eq!(fs::read_to_string(&first.key_path).unwrap(), before);
    }

    #[test]
    fn keygen_rust_literal_matches_the_public_key() {
        let root = Fixture::new();
        let out = keygen(&root, SEED).unwrap();
        let expected = SigningKey::from_bytes(&SEED).verifying_key().to_bytes();
        assert_eq!(out.public_key, expected);

        let lines: Vec<&str> = out.rust_literal.lines().collect();
        assert_eq!(lines.len(), 6, "{}", out.rust_literal);
        assert_eq!(lines[0], "pub const UPDATE_PUBLIC_KEY: [u8; 32] = [");
        assert_eq!(lines[5], "];");

        let mut parsed = Vec::new();
        for row in &lines[1..5] {
            assert!(row.starts_with("    0x"), "{row:?}");
            assert!(row.ends_with(','), "{row:?}");
            let entries: Vec<&str> = row.trim().trim_end_matches(',').split(", ").collect();
            assert_eq!(entries.len(), 8, "{row:?}");
            for entry in entries {
                let hex = entry.strip_prefix("0x").expect("0x prefix");
                assert_eq!(hex.len(), 2, "{entry:?}");
                assert!(
                    hex.chars()
                        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
                    "{entry:?}"
                );
                parsed.push(u8::from_str_radix(hex, 16).unwrap());
            }
        }
        assert_eq!(parsed, expected.to_vec());
    }

    #[test]
    fn keygen_never_contains_the_private_key_in_its_output() {
        let root = Fixture::new();
        let out = keygen(&root, SEED).unwrap();
        let secret = seed_b64(&SEED);
        assert!(!out.rust_literal.contains(&secret));
        assert!(!format!("{out:?}").contains(&secret));
    }
}
