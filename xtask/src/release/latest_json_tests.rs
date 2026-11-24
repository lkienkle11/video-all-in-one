use crate::release::changelog::{read_notes, Notes, VIETNAMESE_TITLE};
use crate::release::latest_json::{add_unique, build_manifest, run};
use crate::release::set_version::fixture::Fixture;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use vaio_core::update::parse_manifest;

pub(crate) const VERSION: &str = "0.1.0";
pub(crate) const BASE: &str = "https://example.test/releases/v0.1.0";
pub(crate) const ARTIFACTS: [&str; 4] = [
    "video-all-in-one-0.1.0-macos-aarch64.zip",
    "video-all-in-one-0.1.0-macos-x86_64.zip",
    "video-all-in-one-0.1.0-windows-x86_64.exe",
    "video-all-in-one-0.1.0-linux-x86_64.deb",
];
pub(crate) const SEED: [u8; 32] = [9; 32];
pub(crate) const OTHER_SEED: [u8; 32] = [10; 32];

pub(crate) fn notes() -> Notes {
    Notes {
        english: "- English notes".to_string(),
        vietnamese: "- Vietnamese notes".to_string(),
    }
}

/// The public key that matches `seed`.
pub(crate) fn public_key(seed: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(seed).verifying_key().to_bytes()
}

/// Writes an artifact and its `.sig` file: the Ed25519 signature of the claim message built
/// from the version and platform in `name`, the size and the SHA-256 digest.
pub(crate) fn signed_artifact(dir: &Path, name: &str, bytes: &[u8], seed: &[u8; 32]) {
    fs::write(dir.join(name), bytes).unwrap();
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    let (platform, version) = crate::release::latest_json::match_artifact(name).unwrap();
    let message = vaio_core::update::signed_message(version, platform, bytes.len() as u64, &digest);
    let signature = SigningKey::from_bytes(seed).sign(&message);
    let encoded = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());
    fs::write(dir.join(format!("{name}.sig")), format!("{encoded}\n")).unwrap();
}

/// Writes an artifact signed with `SEED`.
pub(crate) fn artifact(dir: &Path, name: &str, bytes: &[u8]) {
    signed_artifact(dir, name, bytes, &SEED);
}

/// Writes the four artifacts of `VERSION`, each signed with `SEED`.
pub(crate) fn full_set(dir: &Path) {
    for (index, name) in ARTIFACTS.iter().enumerate() {
        artifact(dir, name, format!("payload {index}").as_bytes());
    }
}

/// The manifest of `dir` with the defaults: `SEED`'s public key and no required platforms.
pub(crate) fn build(dir: &Path) -> anyhow::Result<String> {
    build_manifest(VERSION, &notes(), dir, BASE, &public_key(&SEED), &[])
}

fn build_from_changelog(changelog: &str, dir: &Path) -> Result<String, anyhow::Error> {
    let notes = read_notes(changelog, VERSION)?;
    build_manifest(VERSION, &notes, dir, BASE, &public_key(&SEED), &[])
}

fn changelog_text(english: &str, vietnamese: &str) -> String {
    format!("## {VERSION}\n\n### English\n\n{english}\n\n### {VIETNAMESE_TITLE}\n\n{vietnamese}\n")
}

fn parsed(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn latest_json_contains_both_languages_and_schema_1() {
    let root = Fixture::new();
    full_set(&root);

    let value = parsed(&build(&root).unwrap());

    assert_eq!(value["schema"], 1);
    assert_eq!(value["version"], VERSION);
    assert_eq!(value["notes_en"], "- English notes");
    assert_eq!(value["notes_vi"], "- Vietnamese notes");
    let assets = value["assets"].as_object().unwrap();
    assert_eq!(assets.len(), 4);
    let linux = &assets["linux-x86_64"];
    assert_eq!(
        linux["url"],
        format!("{BASE}/video-all-in-one-0.1.0-linux-x86_64.deb")
    );
    assert!(linux["size"].is_u64());
    assert!(linux["sha256"].is_string());
    assert!(linux["signature"].is_string());
}

#[test]
fn latest_json_fails_when_changelog_section_missing_a_language() {
    let root = Fixture::new();
    full_set(&root);
    let changelog = format!("## {VERSION}\n\n### English\n\nOnly English\n");

    assert!(build_from_changelog(&changelog, &root).is_err());
}

#[test]
fn manifest_is_accepted_by_the_apps_own_parser() {
    let root = Fixture::new();
    full_set(&root);

    let text = build(&root).unwrap();
    let manifest = parse_manifest(&text).unwrap();

    assert_eq!(manifest.version.to_string(), VERSION);
    assert_eq!(manifest.assets.len(), 4);
}

#[test]
fn stale_artifact_version_is_an_error() {
    let root = Fixture::new();
    full_set(&root);
    artifact(&root, "video-all-in-one-0.0.9-linux-x86_64.deb", b"old");

    let error = build(&root).unwrap_err();
    assert!(format!("{error:#}").contains("0.0.9"), "{error:#}");
}

#[test]
fn duplicate_platform_is_an_error() {
    let mut artifacts = BTreeMap::new();
    add_unique(&mut artifacts, "linux-x86_64", ARTIFACTS[3]).unwrap();

    assert!(add_unique(&mut artifacts, "linux-x86_64", "other.deb").is_err());
}

#[test]
fn missing_sig_file_is_an_error() {
    let root = Fixture::new();
    full_set(&root);
    fs::remove_file(root.join("video-all-in-one-0.1.0-windows-x86_64.exe.sig")).unwrap();

    let error = build(&root).unwrap_err();
    assert!(
        format!("{error:#}").contains("video-all-in-one-0.1.0-windows-x86_64.exe"),
        "{error:#}"
    );
}

#[test]
fn bad_signature_length_is_an_error() {
    let root = Fixture::new();
    full_set(&root);
    let short = base64::engine::general_purpose::STANDARD.encode([7u8; 63]);
    fs::write(root.join(format!("{}.sig", ARTIFACTS[0])), short).unwrap();

    assert!(build(&root).is_err());
}

#[test]
fn non_https_base_url_is_an_error() {
    let root = Fixture::new();
    full_set(&root);

    let error = build_manifest(
        VERSION,
        &notes(),
        &root,
        "http://example.test/x",
        &public_key(&SEED),
        &[],
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("https://"), "{error:#}");
}

#[test]
fn unrelated_files_are_ignored() {
    let root = Fixture::new();
    full_set(&root);
    fs::write(
        root.join("video-all-in-one-0.1.0-macos-aarch64.dmg"),
        b"dmg",
    )
    .unwrap();
    fs::write(root.join("notes.txt"), b"notes").unwrap();
    fs::write(root.join("latest.json"), b"{}").unwrap();

    let value = parsed(&build(&root).unwrap());

    assert_eq!(value["assets"].as_object().unwrap().len(), 4);
}

#[test]
fn sha256_and_size_match_the_file() {
    let root = Fixture::new();
    let bytes: Vec<u8> = (0..20_000).map(|i| (i % 253) as u8).collect();
    artifact(&root, ARTIFACTS[3], &bytes);

    let value = parsed(&build(&root).unwrap());

    let linux = &value["assets"]["linux-x86_64"];
    let expected: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(linux["sha256"], expected.as_str());
    assert_eq!(linux["size"], bytes.len() as u64);
}

#[test]
fn no_artifacts_is_an_error() {
    let root = Fixture::new();
    fs::write(root.join("notes.txt"), b"notes").unwrap();

    assert!(build(&root).is_err());
}

#[test]
fn nothing_is_written_on_error() {
    let root = Fixture::new();
    full_set(&root);
    fs::remove_file(root.join(format!("{}.sig", ARTIFACTS[1]))).unwrap();
    fs::write(root.join("CHANGELOG.md"), changelog_text("En", "Vi")).unwrap();
    let out = root.join("out.json");

    let result = run(
        VERSION,
        &root.join("CHANGELOG.md"),
        &root,
        BASE,
        Some(out.as_path()),
        &public_key(&SEED),
        &[],
    );

    assert!(result.is_err());
    assert!(!out.exists());
    assert!(!root.join("latest.json").exists());
}

#[test]
fn run_writes_the_manifest_to_the_default_path() {
    let root = Fixture::new();
    full_set(&root);
    fs::write(root.join("CHANGELOG.md"), changelog_text("En", "Vi")).unwrap();

    run(
        VERSION,
        &root.join("CHANGELOG.md"),
        &root,
        BASE,
        None,
        &public_key(&SEED),
        &[],
    )
    .unwrap();

    let text = fs::read_to_string(root.join("latest.json")).unwrap();
    assert_eq!(parse_manifest(&text).unwrap().assets.len(), 4);
}

#[test]
fn rejects_a_version_with_pre_release_text() {
    let root = Fixture::new();
    full_set(&root);

    let result = build_manifest("0.1.0-beta", &notes(), &root, BASE, &public_key(&SEED), &[]);

    assert!(result.is_err());
}

#[test]
fn signature_made_for_another_version_or_platform_or_size_is_refused() {
    let root = Fixture::new();
    full_set(&root);
    let name = ARTIFACTS[3];
    let bytes = fs::read(root.join(name)).unwrap();
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let size = bytes.len() as u64;
    let cases = [
        ("9.9.9", "linux-x86_64", size),
        ("0.1.0", "windows-x86_64", size),
        ("0.1.0", "linux-x86_64", size + 1),
    ];
    for (version, platform, signed_size) in cases {
        let message = vaio_core::update::signed_message(version, platform, signed_size, &digest);
        let signature = SigningKey::from_bytes(&SEED).sign(&message);
        let encoded = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());
        fs::write(root.join(format!("{name}.sig")), format!("{encoded}\n")).unwrap();
        let error = build(&root).unwrap_err();
        assert!(
            format!("{error:#}").contains("failed signature check"),
            "{version} {platform} {signed_size}: {error:#}"
        );
    }
}
