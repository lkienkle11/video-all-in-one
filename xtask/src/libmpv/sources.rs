//! The one table of source archives the libmpv recipe builds from. Every entry is pinned by an
//! https URL and a SHA-256, and says which licence files of the archive go into the package.
//! To update a component, change its row (see "Our own libmpv" in docs/RELEASING.md).

/// The version of this recipe. It is the number in the release tag `libmpv-<RECIPE_VERSION>` and
/// in the archive names. Change it whenever a row, a flag or a build step changes, so that a new
/// archive never overwrites an old one. The workflow files carry the same number.
pub(crate) const RECIPE_VERSION: &str = "2";

/// One source archive.
pub(crate) struct Source {
    /// The component name used in folder names and in the manifest.
    pub name: &'static str,
    pub version: &'static str,
    /// An https URL of the release archive.
    pub url: &'static str,
    /// SHA-256 of the archive, lowercase hex.
    pub sha256: &'static str,
    /// The local file name of the archive (the cache keeps it under this name).
    pub file: &'static str,
    /// Licence files inside the unpacked archive, copied into `LICENSES`.
    pub licenses: &'static [&'static str],
    /// `Some((parent, folder))` for a source that is not built on its own but unpacked into
    /// `folder` of the unpacked `parent` (the git submodules of the parent's release archive).
    pub nested: Option<(&'static str, &'static str)>,
}

impl Source {
    /// The folder `work/src/...` this source is unpacked into.
    pub(crate) fn src_dir(&self, work: &std::path::Path) -> std::path::PathBuf {
        match self.nested {
            Some((parent, folder)) => work.join("src").join(parent).join(folder),
            None => work.join("src").join(self.name),
        }
    }
}

/// The components in build order: each one only needs the ones before it.
pub(crate) const SOURCES: [Source; 10] = [
    Source {
        name: "openh264",
        version: "2.7.0",
        url: "https://github.com/cisco/openh264/archive/refs/tags/v2.7.0.tar.gz",
        sha256: "5178775c8b440073e70920e97d195efff721c0e56d0a78cc82c919e95d54924d",
        file: "openh264-v2.7.0.tar.gz",
        licenses: &["LICENSE"],
        nested: None,
    },
    Source {
        name: "ffmpeg",
        version: "7.1.5",
        url: "https://ffmpeg.org/releases/ffmpeg-7.1.5.tar.xz",
        sha256: "de668509caf9e35e3cd162473441fdb29538c6d96ed080292b3cf9e6fc5d558f",
        file: "ffmpeg-7.1.5.tar.xz",
        licenses: &["COPYING.LGPLv2.1", "COPYING.LGPLv3", "LICENSE.md"],
        nested: None,
    },
    Source {
        name: "freetype",
        version: "2.13.3",
        url: "https://download-mirror.savannah.gnu.org/releases/freetype/freetype-2.13.3.tar.xz",
        sha256: "0550350666d427c74daeb85d5ac7bb353acba5f76956395995311a9c6f063289",
        file: "freetype-2.13.3.tar.xz",
        licenses: &["docs/FTL.TXT"],
        nested: None,
    },
    Source {
        name: "fribidi",
        version: "1.0.16",
        url: "https://github.com/fribidi/fribidi/releases/download/v1.0.16/fribidi-1.0.16.tar.xz",
        sha256: "1b1cde5b235d40479e91be2f0e88a309e3214c8ab470ec8a2744d82a5a9ea05c",
        file: "fribidi-1.0.16.tar.xz",
        licenses: &["COPYING"],
        nested: None,
    },
    Source {
        name: "harfbuzz",
        version: "11.2.1",
        url: "https://github.com/harfbuzz/harfbuzz/releases/download/11.2.1/harfbuzz-11.2.1.tar.xz",
        sha256: "093714c8548a285094685f0bdc999e202d666b59eeb3df2ff921ab68b8336a49",
        file: "harfbuzz-11.2.1.tar.xz",
        licenses: &["COPYING"],
        nested: None,
    },
    Source {
        name: "libass",
        version: "0.17.4",
        url: "https://github.com/libass/libass/releases/download/0.17.4/libass-0.17.4.tar.xz",
        sha256: "78f1179b838d025e9c26e8fef33f8092f65611444ffa1bfc0cfac6a33511a05a",
        file: "libass-0.17.4.tar.xz",
        licenses: &["COPYING"],
        nested: None,
    },
    Source {
        name: "libplacebo",
        version: "7.349.0",
        url: "https://code.videolan.org/videolan/libplacebo/-/archive/v7.349.0/libplacebo-v7.349.0.tar.gz",
        sha256: "79120e685a1836344b51b13b6a5661622486a84e4d4a35f6c8d01679a20fbc86",
        file: "libplacebo-7.349.0.tar.gz",
        licenses: &["LICENSE"],
        nested: None,
    },
    Source {
        name: "fast_float",
        version: "8.0.2",
        url: "https://github.com/fastfloat/fast_float/archive/refs/tags/v8.0.2.tar.gz",
        sha256: "e14a33089712b681d74d94e2a11362643bd7d769ae8f7e7caefe955f57f7eacd",
        file: "fast_float-8.0.2.tar.gz",
        licenses: &["LICENSE-MIT", "LICENSE-APACHE"],
        nested: Some(("libplacebo", "3rdparty/fast_float")),
    },
    Source {
        name: "vulkan-headers",
        version: "1.4.309",
        url: "https://github.com/KhronosGroup/Vulkan-Headers/archive/refs/tags/v1.4.309.tar.gz",
        sha256: "437925ada160d86ed763d29dcb9318c1bb0d024d7deaf77bc7c170b8eb6b6f10",
        file: "vulkan-headers-1.4.309.tar.gz",
        licenses: &["LICENSE.md"],
        nested: Some(("libplacebo", "3rdparty/Vulkan-Headers")),
    },
    Source {
        name: "mpv",
        version: "0.40.0",
        url: "https://github.com/mpv-player/mpv/archive/refs/tags/v0.40.0.tar.gz",
        sha256: "10a0f4654f62140a6dd4d380dcf0bbdbdcf6e697556863dc499c296182f081a3",
        file: "mpv-0.40.0.tar.gz",
        licenses: &["LICENSE.LGPL", "Copyright"],
        nested: None,
    },
];

/// The row named `name`.
#[cfg(test)]
pub(crate) fn source(name: &str) -> &'static Source {
    SOURCES
        .iter()
        .find(|source| source.name == name)
        .unwrap_or_else(|| panic!("no source named {name} in SOURCES"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    #[test]
    fn openh264_source_is_pinned_with_a_sha256() {
        let openh264 = source("openh264");
        assert_eq!(openh264.version, "2.7.0");
        assert_eq!(
            openh264.url,
            "https://github.com/cisco/openh264/archive/refs/tags/v2.7.0.tar.gz"
        );
        assert_eq!(
            openh264.sha256,
            "5178775c8b440073e70920e97d195efff721c0e56d0a78cc82c919e95d54924d"
        );
        assert_eq!(openh264.file, "openh264-v2.7.0.tar.gz");
        assert_eq!(openh264.licenses, &["LICENSE"]);
        let position = |name| SOURCES.iter().position(|s| s.name == name).unwrap();
        assert!(position("openh264") < position("ffmpeg"));
    }

    #[test]
    fn recipe_version_matches_workflows() {
        assert_eq!(RECIPE_VERSION, "2");
        let workflows = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows");
        for name in ["build-libmpv.yml", "ci.yml", "release.yml"] {
            let text = std::fs::read_to_string(workflows.join(name)).unwrap();
            assert!(text.contains("LIBMPV_RECIPE: \"2\""), "{name}");
            if name != "build-libmpv.yml" {
                assert!(text.contains("/releases/download/libmpv-2"), "{name}");
                for arch in ["MACOS_ARM64", "MACOS_X86_64", "LINUX_X86_64"] {
                    let prefix = format!("LIBMPV_SHA256_{arch}: \"");
                    let hash = text
                        .lines()
                        .find_map(|line| line.trim_start().strip_prefix(prefix.as_str()))
                        .and_then(|rest| rest.strip_suffix('"'))
                        .unwrap_or_else(|| panic!("{name} lacks {prefix}"));
                    assert!(
                        hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
                        "{name} {arch} is not a pinned SHA-256: {hash}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_entry_has_a_64_hex_sha256_and_an_https_url() {
        for source in &SOURCES {
            assert_eq!(source.sha256.len(), 64, "{}", source.name);
            assert!(
                source
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "{} hash must be lowercase hex",
                source.name
            );
            assert!(source.url.starts_with("https://"), "{}", source.name);
            assert!(
                source.url.contains(source.version),
                "{} url must carry its version",
                source.name
            );
            assert!(
                source.file.ends_with(".tar.xz") || source.file.ends_with(".tar.gz"),
                "{}",
                source.name
            );
            assert!(
                !source.licenses.is_empty(),
                "{} needs a licence",
                source.name
            );
        }
    }

    #[test]
    fn names_hashes_and_files_are_unique() {
        let names: BTreeSet<_> = SOURCES.iter().map(|s| s.name).collect();
        let hashes: BTreeSet<_> = SOURCES.iter().map(|s| s.sha256).collect();
        let files: BTreeSet<_> = SOURCES.iter().map(|s| s.file).collect();
        assert_eq!(names.len(), SOURCES.len());
        assert_eq!(hashes.len(), SOURCES.len());
        assert_eq!(files.len(), SOURCES.len());
    }

    #[test]
    fn nested_sources_name_a_parent_that_is_built_before_them() {
        for (index, source) in SOURCES.iter().enumerate() {
            if let Some((parent, folder)) = source.nested {
                let at = SOURCES.iter().position(|s| s.name == parent).unwrap();
                assert!(at < index, "{} comes before its parent", source.name);
                assert!(SOURCES[at].nested.is_none());
                assert!(!folder.starts_with('/') && !folder.contains(".."));
            }
        }
    }

    #[test]
    fn mpv_and_ffmpeg_are_present_and_mpv_comes_last() {
        assert_eq!(source("ffmpeg").name, "ffmpeg");
        assert_eq!(SOURCES.last().unwrap().name, "mpv");
        assert_eq!(SOURCES.first().unwrap().name, "openh264");
    }

    /// The workflow files repeat the recipe version; they must agree with the code.
    #[test]
    fn workflows_carry_the_recipe_version_of_the_code() {
        let workflows = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows");
        let expected = format!("LIBMPV_RECIPE: \"{RECIPE_VERSION}\"");
        for name in ["build-libmpv.yml", "ci.yml", "release.yml"] {
            let text = std::fs::read_to_string(workflows.join(name)).unwrap();
            assert!(text.contains(&expected), "{name} must contain {expected}");
        }
    }
}
