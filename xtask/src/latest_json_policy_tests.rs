use crate::latest_json::{build_manifest, decode_public_key};
use crate::latest_json_tests::{
    artifact, build, full_set, notes, public_key, signed_artifact, ARTIFACTS, BASE, OTHER_SEED,
    SEED, VERSION,
};
use crate::set_version::fixture::Fixture;
use base64::Engine;
use std::fs;
use std::path::Path;

const ALL_PLATFORMS: [&str; 4] = [
    "macos-aarch64",
    "macos-x86_64",
    "windows-x86_64",
    "linux-x86_64",
];

fn build_at(dir: &Path, base_url: &str) -> anyhow::Result<String> {
    build_manifest(VERSION, &notes(), dir, base_url, &public_key(&SEED), &[])
}

fn build_requiring(dir: &Path, required: &[&str]) -> anyhow::Result<String> {
    let required: Vec<String> = required.iter().map(|key| key.to_string()).collect();
    build_manifest(VERSION, &notes(), dir, BASE, &public_key(&SEED), &required)
}

#[test]
fn public_key_must_decode_to_32_bytes() {
    let good = base64::engine::general_purpose::STANDARD.encode(public_key(&SEED));
    assert_eq!(decode_public_key(&good).unwrap(), public_key(&SEED));

    let short = base64::engine::general_purpose::STANDARD.encode([1u8; 31]);
    assert!(decode_public_key(&short).is_err());
    assert!(decode_public_key("!!not base64!!").is_err());
}

#[test]
fn valid_signatures_are_accepted() {
    let root = Fixture::new();
    full_set(&root);

    let text = build(&root).unwrap();

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).unwrap()["assets"]
            .as_object()
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn signature_of_another_file_is_rejected() {
    let root = Fixture::new();
    full_set(&root);
    fs::copy(
        root.join(format!("{}.sig", ARTIFACTS[3])),
        root.join(format!("{}.sig", ARTIFACTS[2])),
    )
    .unwrap();

    let error = build(&root).unwrap_err();

    let text = format!("{error:#}");
    assert!(text.contains(ARTIFACTS[2]), "{text}");
    assert!(text.contains("signature"), "{text}");
}

#[test]
fn signature_from_another_key_is_rejected() {
    let root = Fixture::new();
    for (index, name) in ARTIFACTS.iter().enumerate() {
        signed_artifact(
            &root,
            name,
            format!("payload {index}").as_bytes(),
            &OTHER_SEED,
        );
    }

    let error = build(&root).unwrap_err();

    assert!(format!("{error:#}").contains("signature"), "{error:#}");
}

#[test]
fn corrupted_signature_is_rejected() {
    let root = Fixture::new();
    full_set(&root);
    let sig_path = root.join(format!("{}.sig", ARTIFACTS[0]));
    let mut bytes = base64::engine::general_purpose::STANDARD
        .decode(fs::read_to_string(&sig_path).unwrap().trim())
        .unwrap();
    bytes[0] ^= 0x01;
    let corrupted = base64::engine::general_purpose::STANDARD.encode(bytes);
    fs::write(&sig_path, format!("{corrupted}\n")).unwrap();

    let error = build(&root).unwrap_err();

    assert!(format!("{error:#}").contains(ARTIFACTS[0]), "{error:#}");
}

#[test]
fn missing_required_platform_is_an_error() {
    let root = Fixture::new();
    artifact(&root, ARTIFACTS[3], b"linux");

    let error =
        build_requiring(&root, &["macos-x86_64", "macos-aarch64", "linux-x86_64"]).unwrap_err();

    let text = format!("{error:#}");
    assert!(text.contains("macos-aarch64"), "{text}");
    assert!(text.contains("macos-x86_64"), "{text}");
    assert!(!text.contains("linux-x86_64"), "{text}");
}

#[test]
fn all_required_platforms_present_is_accepted() {
    let root = Fixture::new();
    full_set(&root);

    assert!(build_requiring(&root, &ALL_PLATFORMS).is_ok());
}

#[test]
fn empty_required_list_keeps_the_old_behavior() {
    let root = Fixture::new();
    artifact(&root, ARTIFACTS[3], b"linux");

    let text = build_requiring(&root, &[]).unwrap();

    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["assets"].as_object().unwrap().len(), 1);
}

#[test]
fn unknown_required_platform_is_an_error() {
    let root = Fixture::new();
    full_set(&root);

    let error = build_requiring(&root, &["solaris-sparc"]).unwrap_err();

    assert!(format!("{error:#}").contains("solaris-sparc"), "{error:#}");
}

#[test]
fn base_url_with_credentials_is_rejected() {
    let root = Fixture::new();
    full_set(&root);

    assert!(build_at(&root, "https://user:secret@example.test/x").is_err());
}

#[test]
fn base_url_with_query_or_fragment_is_rejected() {
    let root = Fixture::new();
    full_set(&root);

    assert!(build_at(&root, "https://example.test/x?a=1").is_err());
    assert!(build_at(&root, "https://example.test/x#top").is_err());
}

#[test]
fn base_url_without_host_is_rejected() {
    let root = Fixture::new();
    full_set(&root);

    assert!(build_at(&root, "https:///x").is_err());
    assert!(build_at(&root, "https://").is_err());
}

#[test]
fn normal_github_release_url_is_accepted() {
    let root = Fixture::new();
    full_set(&root);

    let text = build_at(
        &root,
        "https://github.com/owner/repo/releases/download/v0.1.0",
    )
    .unwrap();

    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["assets"].as_object().unwrap().len(), 4);
}
