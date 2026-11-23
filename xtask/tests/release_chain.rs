use base64::Engine;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use vaio_core::update::parse_manifest;
use vaio_platform::update::verify::verify_artifact;

static NEXT: AtomicUsize = AtomicUsize::new(0);
const KEY_ENV: &str = "UPDATE_SIGNING_KEY";
const VERSION: &str = "0.1.0";
const BASE: &str = "https://example.test/releases/v0.1.0";
const REQUIRED: &str = "macos-aarch64,windows-x86_64,linux-x86_64";
const SIGNED: [&str; 3] = [
    "video-all-in-one-0.1.0-macos-aarch64.zip",
    "video-all-in-one-0.1.0-windows-x86_64.exe",
    "video-all-in-one-0.1.0-linux-x86_64.deb",
];

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "vaio-release-chain-{}-{}",
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

fn xtask(args: &[&str], key: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command.args(args).env_remove(KEY_ENV);
    if let Some(value) = key {
        command.env(KEY_ENV, value);
    }
    command.output().expect("running xtask")
}

fn ok(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Runs `keygen` into `<dir>/<name>` and returns (private key, public key), both base64.
fn keygen(dir: &TempDir, name: &str) -> (String, String) {
    let keys = dir.0.join(name);
    ok(&xtask(&["keygen", "--out", keys.to_str().unwrap()], None));
    let secret = fs::read_to_string(keys.join("update-signing.key")).unwrap();
    let public = fs::read_to_string(keys.join("update-signing.pub")).unwrap();
    (secret.trim().to_string(), public.trim().to_string())
}

/// Writes the three artifacts of `VERSION`, one per required platform, and returns their folder.
fn artifacts(dir: &TempDir) -> PathBuf {
    let assets = dir.0.join("assets");
    fs::create_dir_all(&assets).unwrap();
    for name in SIGNED {
        fs::write(assets.join(name), format!("bytes of {name}")).unwrap();
    }
    assets
}

fn write_changelog(dir: &TempDir) -> PathBuf {
    let path = dir.0.join("CHANGELOG.md");
    fs::write(
        &path,
        format!("## {VERSION}\n\n### English\n\n- Notes\n\n### Tiếng Việt\n\n- Ghi chú\n"),
    )
    .unwrap();
    path
}

fn sign_all(assets: &Path, secret: &str) {
    for name in SIGNED {
        let path = assets.join(name);
        ok(&xtask(
            &["sign", "--file", path.to_str().unwrap()],
            Some(secret),
        ));
    }
}

fn latest_json(dir: &TempDir, assets: &Path, public: &str) -> Output {
    let changelog = write_changelog(dir);
    xtask(
        &[
            "latest-json",
            "--version",
            VERSION,
            "--notes",
            changelog.to_str().unwrap(),
            "--assets",
            assets.to_str().unwrap(),
            "--base-url",
            BASE,
            "--public-key",
            public,
            "--require",
            REQUIRED,
        ],
        None,
    )
}

#[test]
fn keygen_sign_latest_json_and_verify_the_chain() {
    let dir = TempDir::new();
    let (secret, public) = keygen(&dir, "keys");
    let assets = artifacts(&dir);
    sign_all(&assets, &secret);

    ok(&latest_json(&dir, &assets, &public));

    let text = fs::read_to_string(assets.join("latest.json")).unwrap();
    let manifest = parse_manifest(&text).unwrap();
    assert_eq!(manifest.assets.len(), 3);
    let linux = &manifest.assets["linux-x86_64"];
    let bytes = fs::read(assets.join(SIGNED[2])).unwrap();
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let public_bytes: [u8; 32] = base64::engine::general_purpose::STANDARD
        .decode(&public)
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(
        verify_artifact(digest, &linux.sha256, &linux.signature, &public_bytes),
        Ok(())
    );
}

#[test]
fn manifest_built_with_another_public_key_is_refused() {
    let dir = TempDir::new();
    let (secret_a, _) = keygen(&dir, "a");
    let (_, public_b) = keygen(&dir, "b");
    let assets = artifacts(&dir);
    sign_all(&assets, &secret_a);

    let output = latest_json(&dir, &assets, &public_b);

    assert!(!output.status.success());
    assert!(!assets.join("latest.json").exists());
}
