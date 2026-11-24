use super::{check_changelog, package_file_name, platform_key, update_extension, Target};
use crate::release::changelog::VIETNAMESE_TITLE;
use crate::release::latest_json::match_artifact;

#[test]
fn package_names_follow_version_and_target() {
    assert_eq!(
        package_file_name("0.1.0", "macos-aarch64", "zip"),
        "video-all-in-one-0.1.0-macos-aarch64.zip"
    );
    assert_eq!(
        package_file_name("0.1.0", "macos-aarch64", "dmg"),
        "video-all-in-one-0.1.0-macos-aarch64.dmg"
    );
    assert_eq!(
        platform_key(Target::Macos, "aarch64").unwrap(),
        "macos-aarch64"
    );
    assert_eq!(
        platform_key(Target::Macos, "x86_64").unwrap(),
        "macos-x86_64"
    );
    assert_eq!(
        platform_key(Target::Windows, "x86_64").unwrap(),
        "windows-x86_64"
    );
    assert_eq!(
        platform_key(Target::Ubuntu, "x86_64").unwrap(),
        "linux-x86_64"
    );
    assert!(platform_key(Target::Windows, "aarch64").is_err());
    assert!(platform_key(Target::Ubuntu, "aarch64").is_err());
    assert_eq!(update_extension("macos-x86_64").unwrap(), "zip");
    assert_eq!(update_extension("windows-x86_64").unwrap(), "exe");
    assert_eq!(update_extension("linux-x86_64").unwrap(), "deb");
    assert!(update_extension("plan9-mips").is_err());
}

#[test]
fn update_artifact_names_are_the_names_latest_json_looks_for() {
    for key in [
        "macos-aarch64",
        "macos-x86_64",
        "windows-x86_64",
        "linux-x86_64",
    ] {
        let name = package_file_name("1.2.3", key, update_extension(key).unwrap());
        assert_eq!(match_artifact(&name), Some((key, "1.2.3")), "{name}");
    }
    // The macOS disk image is a download for people, never an update artifact.
    let dmg = package_file_name("1.2.3", "macos-aarch64", "dmg");
    assert_eq!(match_artifact(&dmg), None);
}

#[test]
fn changelog_section_for_version_has_both_languages() {
    let text = format!(
        "# Changelog\n\n## 0.1.0\n\n### English\n\n- First release\n\n### {VIETNAMESE_TITLE}\n\n- Ban dau\n\n## 0.0.9\n\nold\n"
    );
    let notes = check_changelog(&text, "0.1.0").unwrap();
    assert_eq!(notes.english, "- First release");
    assert_eq!(notes.vietnamese, "- Ban dau");

    let no_vietnamese = "## 0.1.0\n\n### English\n\n- First release\n";
    let message = format!("{:#}", check_changelog(no_vietnamese, "0.1.0").unwrap_err());
    assert!(message.contains(VIETNAMESE_TITLE), "{message}");

    let other_version = format!("## 0.2.0\n\n### English\n\nEn\n\n### {VIETNAMESE_TITLE}\n\nVi\n");
    assert!(check_changelog(&other_version, "0.1.0").is_err());
    assert!(
        check_changelog(&text, "0.1").is_err(),
        "a version must be X.Y.Z"
    );
}
