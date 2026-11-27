use super::{check_changelog, package_file_name, platform_key, update_extension, Target};
use crate::release::changelog::VIETNAMESE_TITLE;
use crate::release::latest_json::match_artifact;

#[test]
fn tools_reach_frameworks_and_library_dependencies_stay_loader_relative() {
    use super::macos_libs::install_name_args;
    use std::path::Path;
    let tool = install_name_args(
        None,
        &["@rpath/libavcodec.dylib".into()],
        Path::new("App.app/Contents/Resources/bin/ffmpeg"),
    );
    assert!(tool
        .windows(2)
        .any(|args| args == ["-add_rpath", "@loader_path/../../Frameworks"]));
    assert!(tool.iter().any(|arg| arg == "@rpath/libavcodec.dylib"));
    let library = install_name_args(
        Some("libavcodec.dylib"),
        &["@rpath/libavutil.dylib".into()],
        Path::new("App.app/Contents/Frameworks/libavcodec.dylib"),
    );
    assert!(library
        .iter()
        .any(|arg| arg == "@loader_path/libavutil.dylib"));
}

pub(super) struct ToolFixture(pub std::path::PathBuf);

impl ToolFixture {
    pub(super) fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "vaio-package-tools-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(path.join("bin")).unwrap();
        std::fs::create_dir_all(path.join("LICENSES")).unwrap();
        std::fs::write(path.join("LICENSES/SOURCES.txt"), b"fixture sources").unwrap();
        for name in ["ffmpeg", "ffprobe", "ffmpeg.exe", "ffprobe.exe", "app"] {
            std::fs::write(path.join(name), b"fixture tool").unwrap();
        }
        for name in ["ffmpeg", "ffprobe"] {
            std::fs::write(path.join("bin").join(name), b"fixture tool").unwrap();
        }
        Self(path)
    }
}

impl Drop for ToolFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn deb_control_has_no_ffmpeg_recommendation() {
    let text = super::deb::control_file("1.2.3", 42, &["libc6".into()]);
    assert!(
        !text.lines().any(|line| line.starts_with("Recommends:")),
        "{text}"
    );
}

#[test]
fn deb_lists_the_bundled_tools() {
    let dir = ToolFixture::new();
    let _ = super::deb::build(
        &dir.0,
        &dir.0.join("app"),
        &dir.0.join("out"),
        "1.2.3",
        "linux-x86_64",
        &super::deb::Bundle {
            libmpv_dir: &dir.0,
            allow_gpl: false,
        },
    );
    for name in ["ffmpeg", "ffprobe"] {
        let path = dir
            .0
            .join("out/stage-deb/video-all-in-one_1.2.3_amd64/usr/lib/video-all-in-one/bin")
            .join(name);
        assert!(path.is_file(), "missing {}", path.display());
        assert_eq!(std::fs::read(path).unwrap(), b"fixture tool");
    }
}

#[test]
fn macos_bundle_contains_resources_bin_tools() {
    let dir = ToolFixture::new();
    let _ = super::macos::build(
        &dir.0,
        &dir.0.join("app"),
        &dir.0.join("out"),
        "1.2.3",
        "macos-aarch64",
        false,
        Some(&dir.0),
    );
    for name in ["ffmpeg", "ffprobe"] {
        let path = dir
            .0
            .join("out/stage-macos/Video All-in-One.app/Contents/Resources/bin")
            .join(name);
        assert!(path.is_file(), "missing {}", path.display());
        assert_eq!(std::fs::read(path).unwrap(), b"fixture tool");
    }
}

#[test]
fn nsis_script_installs_bin_tools() {
    use std::path::Path;
    let script = super::nsis::installer_script(
        "1.2.3",
        Path::new("installer.exe"),
        Path::new("stage"),
        &["video-all-in-one.exe".into(), "libmpv-2.dll".into()],
        Path::new("icon.ico"),
    )
    .unwrap();
    assert!(script.contains("SetOutPath \"$INSTDIR\\bin\""), "{script}");
    for name in ["ffmpeg.exe", "ffprobe.exe"] {
        assert!(
            script.contains(&format!("File \"stage\\bin\\{name}\"")),
            "{script}"
        );
        assert!(
            script.contains(&format!("Delete \"$INSTDIR\\bin\\{name}\"")),
            "{script}"
        );
    }
    assert!(script.contains("RMDir \"$INSTDIR\\bin\""));
    assert!(script.contains("Delete \"$INSTDIR\\bin\\libmpv-2.dll\""));
    assert!(!script.contains("RMDir /r"));
}

#[test]
fn nsis_script_carries_and_removes_license_and_source_notices() {
    use std::path::Path;
    let script = super::nsis::installer_script(
        "1.2.3",
        Path::new("installer.exe"),
        Path::new("stage"),
        &[
            "video-all-in-one.exe".into(),
            "third-party\\SOURCES.txt".into(),
            "third-party\\openh264-LICENSE".into(),
        ],
        Path::new("icon.ico"),
    )
    .unwrap();
    assert!(script.contains("SetOutPath \"$INSTDIR\\third-party\""));
    for name in ["SOURCES.txt", "openh264-LICENSE"] {
        assert!(script.contains(&format!("File \"stage\\third-party\\{name}\"")));
        assert!(script.contains(&format!("Delete \"$INSTDIR\\third-party\\{name}\"")));
    }
    assert!(script.contains("RMDir \"$INSTDIR\\third-party\""));
}

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
