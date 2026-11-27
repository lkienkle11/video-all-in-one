use super::{info_plist, resolve_in_build};
use std::path::Path;
use vaio_platform::channel::Channel;

#[test]
fn libraries_resolve_only_inside_the_libmpv_build() {
    let dir = std::env::temp_dir().join(format!("vaio-macos-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("libmpv.2.dylib"), b"x").unwrap();
    let real = dir.canonicalize().unwrap().join("libmpv.2.dylib");
    assert_eq!(
        resolve_in_build("@rpath/libmpv.2.dylib", Path::new(""), &dir).unwrap(),
        real
    );
    assert_eq!(
        resolve_in_build("@loader_path/libmpv.2.dylib", &dir, &dir).unwrap(),
        real
    );
    for outside in [
        "/opt/homebrew/lib/libmpv.2.dylib",
        "/usr/local/lib/libx.dylib",
        "@executable_path/x.dylib",
    ] {
        let error = resolve_in_build(outside, &dir, &dir)
            .unwrap_err()
            .to_string();
        assert!(error.contains("outside the libmpv build"), "{error}");
    }
    assert!(resolve_in_build("@rpath/libmissing.dylib", Path::new(""), &dir).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn info_plist_names_the_production_app_and_its_minimum_system() {
    let plist = info_plist(Channel::Production, "0.1.0", "13.0");
    for expected in [
        "<key>CFBundleIdentifier</key>\n\t<string>app.videoallinone</string>",
        "<key>CFBundleExecutable</key>\n\t<string>video-all-in-one</string>",
        "<key>CFBundleShortVersionString</key>\n\t<string>0.1.0</string>",
        "<key>CFBundleVersion</key>\n\t<string>0.1.0</string>",
        "<key>LSMinimumSystemVersion</key>\n\t<string>13.0</string>",
        "<key>CFBundleName</key>\n\t<string>Video All-in-One</string>",
        "<key>CFBundlePackageType</key>\n\t<string>APPL</string>",
        "<key>CFBundleIconFile</key>\n\t<string>AppIcon</string>",
    ] {
        assert!(plist.contains(expected), "missing {expected:?} in\n{plist}");
    }
    assert!(plist.starts_with("<?xml"));
    assert!(
        !plist.contains("app.videoallinone.dev"),
        "never the dev identifier"
    );
}

#[test]
fn info_plist_escapes_text_it_embeds() {
    assert!(info_plist(Channel::Production, "1.0.0<&>", "13.0").contains("1.0.0&lt;&amp;&gt;"));
}
