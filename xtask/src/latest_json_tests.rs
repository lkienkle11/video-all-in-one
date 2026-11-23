use crate::changelog::{read_notes, Notes, VIETNAMESE_TITLE};
use crate::latest_json::{add_unique, build_manifest, run};
use crate::set_version::fixture::Fixture;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use vaio_core::update::parse_manifest;

const VERSION: &str = "0.1.0";
const BASE: &str = "https://example.test/releases/v0.1.0";
const ARTIFACTS: [&str; 4] = [
    "video-all-in-one-0.1.0-macos-aarch64.zip",
    "video-all-in-one-0.1.0-macos-x86_64.zip",
    "video-all-in-one-0.1.0-windows-x86_64.exe",
    "video-all-in-one-0.1.0-linux-x86_64.deb",
];

fn notes() -> Notes {
    Notes {
        english: "- English notes".to_string(),
        vietnamese: "- Vietnamese notes".to_string(),
    }
}

fn signature_text() -> String {
    base64::engine::general_purpose::STANDARD.encode([7u8; 64])
}

/// Writes an artifact with its `.sig` file next to it.
fn artifact(dir: &Path, name: &str, bytes: &[u8]) {
    fs::write(dir.join(name), bytes).unwrap();
    fs::write(
        dir.join(format!("{name}.sig")),
        format!("{}\n", signature_text()),
    )
    .unwrap();
}

/// Writes the four artifacts of `VERSION`, each with a `.sig` file.
fn full_set(dir: &Path) {
    for (index, name) in ARTIFACTS.iter().enumerate() {
        artifact(dir, name, format!("payload {index}").as_bytes());
    }
}

fn build_from_changelog(changelog: &str, dir: &Path) -> Result<String, anyhow::Error> {
    let notes = read_notes(changelog, VERSION)?;
    build_manifest(VERSION, &notes, dir, BASE)
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

    let value = parsed(&build_manifest(VERSION, &notes(), &root, BASE).unwrap());

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

    let text = build_manifest(VERSION, &notes(), &root, BASE).unwrap();
    let manifest = parse_manifest(&text).unwrap();

    assert_eq!(manifest.version.to_string(), VERSION);
    assert_eq!(manifest.assets.len(), 4);
}

#[test]
fn stale_artifact_version_is_an_error() {
    let root = Fixture::new();
    full_set(&root);
    artifact(&root, "video-all-in-one-0.0.9-linux-x86_64.deb", b"old");

    let error = build_manifest(VERSION, &notes(), &root, BASE).unwrap_err();
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

    let error = build_manifest(VERSION, &notes(), &root, BASE).unwrap_err();
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

    assert!(build_manifest(VERSION, &notes(), &root, BASE).is_err());
}

#[test]
fn non_https_base_url_is_an_error() {
    let root = Fixture::new();
    full_set(&root);

    let error = build_manifest(VERSION, &notes(), &root, "http://example.test/x").unwrap_err();
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

    let value = parsed(&build_manifest(VERSION, &notes(), &root, BASE).unwrap());

    assert_eq!(value["assets"].as_object().unwrap().len(), 4);
}

#[test]
fn sha256_and_size_match_the_file() {
    let root = Fixture::new();
    let bytes: Vec<u8> = (0..20_000).map(|i| (i % 253) as u8).collect();
    artifact(&root, ARTIFACTS[3], &bytes);

    let value = parsed(&build_manifest(VERSION, &notes(), &root, BASE).unwrap());

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

    assert!(build_manifest(VERSION, &notes(), &root, BASE).is_err());
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

    run(VERSION, &root.join("CHANGELOG.md"), &root, BASE, None).unwrap();

    let text = fs::read_to_string(root.join("latest.json")).unwrap();
    assert_eq!(parse_manifest(&text).unwrap().assets.len(), 4);
}

#[test]
fn rejects_a_version_with_pre_release_text() {
    let root = Fixture::new();
    full_set(&root);

    let result = build_manifest("0.1.0-beta", &notes(), &root, BASE);

    assert!(result.is_err());
}
