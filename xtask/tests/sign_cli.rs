use base64::Engine;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use vaio_platform::update::verify::verify_artifact;

static NEXT: AtomicUsize = AtomicUsize::new(0);
const SEED: [u8; 32] = [21; 32];
const KEY_ENV: &str = "UPDATE_SIGNING_KEY";

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "vaio-sign-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn key_b64() -> String {
    base64::engine::general_purpose::STANDARD.encode(SEED)
}

fn xtask(args: &[&str], key: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command.args(args).env_remove(KEY_ENV);
    if let Some(value) = key {
        command.env(KEY_ENV, value);
    }
    command.output().expect("running xtask")
}

fn artifact(dir: &TempDir) -> PathBuf {
    let path = dir.0.join("artifact.bin");
    fs::write(&path, b"update bytes").unwrap();
    path
}

fn assert_no_key(output: &Output, secret: &str) {
    assert!(!String::from_utf8_lossy(&output.stdout).contains(secret));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(secret));
}

/// Reads the `0x..` bytes printed by `keygen` (the public key literal).
fn parse_public_key(text: &str) -> [u8; 32] {
    let bytes: Vec<u8> = text
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|token| token.strip_prefix("0x"))
        .map(|hex| u8::from_str_radix(hex, 16).unwrap())
        .collect();
    bytes.try_into().expect("32 public key bytes")
}

#[test]
fn sign_never_prints_private_key() {
    let dir = TempDir::new();
    let file = artifact(&dir);
    let file_arg = file.to_str().unwrap();
    let key = key_b64();

    let ok = xtask(&["sign", "--file", file_arg], Some(&key));
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let stdout = String::from_utf8(ok.stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "{stdout}");
    assert!(lines[0].starts_with("sha256=") && lines[0].len() == "sha256=".len() + 64);
    assert!(lines[1].starts_with("signature="));
    assert_no_key(&ok, &key);

    let missing = dir.0.join("missing.bin");
    let out = xtask(&["sign", "--file", missing.to_str().unwrap()], Some(&key));
    assert!(!out.status.success());
    assert_no_key(&out, &key);

    let bad = "!!not-base64-value!!";
    let out = xtask(&["sign", "--file", file_arg], Some(bad));
    assert!(!out.status.success());
    assert_no_key(&out, bad);
}

#[test]
fn sign_without_the_environment_variable_fails() {
    let dir = TempDir::new();
    let file = artifact(&dir);
    let out = xtask(&["sign", "--file", file.to_str().unwrap()], None);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(!dir.0.join("artifact.bin.sig").exists());
}

#[test]
fn keygen_then_sign_then_verify_end_to_end() {
    let dir = TempDir::new();
    let keys = dir.0.join("keys");
    let keygen = xtask(&["keygen", "--out", keys.to_str().unwrap()], None);
    assert!(
        keygen.status.success(),
        "{}",
        String::from_utf8_lossy(&keygen.stderr)
    );
    let secret = fs::read_to_string(keys.join("update-signing.key"))
        .unwrap()
        .trim()
        .to_string();
    assert_no_key(&keygen, &secret);
    let public_key = parse_public_key(&String::from_utf8(keygen.stdout.clone()).unwrap());

    let file = artifact(&dir);
    let signed = xtask(&["sign", "--file", file.to_str().unwrap()], Some(&secret));
    assert!(
        signed.status.success(),
        "{}",
        String::from_utf8_lossy(&signed.stderr)
    );
    let stdout = String::from_utf8(signed.stdout.clone()).unwrap();
    let sha = stdout
        .lines()
        .find_map(|l| l.strip_prefix("sha256="))
        .unwrap();
    let sig = stdout
        .lines()
        .find_map(|l| l.strip_prefix("signature="))
        .unwrap();
    let digest: [u8; 32] = Sha256::digest(fs::read(&file).unwrap()).into();
    assert_eq!(verify_artifact(digest, sha, sig, &public_key), Ok(()));
}
