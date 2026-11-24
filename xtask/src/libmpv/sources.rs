//! The one table of source archives the libmpv recipe builds from. Every entry is pinned by an
//! https URL and a SHA-256, and says which licence files of the archive go into the package.
//! To update a component, change its row (see "Our own libmpv" in docs/RELEASING.md).

/// The version of this recipe. It is the number in the release tag `libmpv-<RECIPE_VERSION>` and
/// in the archive names. Change it whenever a row, a flag or a build step changes, so that a new
/// archive never overwrites an old one. The workflow files carry the same number.
pub(crate) const RECIPE_VERSION: &str = "1";

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
}

/// The components in build order: each one only needs the ones before it.
pub(crate) const SOURCES: [Source; 7] = [
    Source {
        name: "ffmpeg",
        version: "7.1.5",
        url: "https://ffmpeg.org/releases/ffmpeg-7.1.5.tar.xz",
        sha256: "de668509caf9e35e3cd162473441fdb29538c6d96ed080292b3cf9e6fc5d558f",
        file: "ffmpeg-7.1.5.tar.xz",
        licenses: &["COPYING.LGPLv2.1", "COPYING.LGPLv3", "LICENSE.md"],
    },
    Source {
        name: "freetype",
        version: "2.13.3",
        url: "https://download-mirror.savannah.gnu.org/releases/freetype/freetype-2.13.3.tar.xz",
        sha256: "0550350666d427c74daeb85d5ac7bb353acba5f76956395995311a9c6f063289",
        file: "freetype-2.13.3.tar.xz",
        licenses: &["docs/FTL.TXT"],
    },
    Source {
        name: "fribidi",
        version: "1.0.16",
        url: "https://github.com/fribidi/fribidi/releases/download/v1.0.16/fribidi-1.0.16.tar.xz",
        sha256: "1b1cde5b235d40479e91be2f0e88a309e3214c8ab470ec8a2744d82a5a9ea05c",
        file: "fribidi-1.0.16.tar.xz",
        licenses: &["COPYING"],
    },
    Source {
        name: "harfbuzz",
        version: "11.2.1",
        url: "https://github.com/harfbuzz/harfbuzz/releases/download/11.2.1/harfbuzz-11.2.1.tar.xz",
        sha256: "093714c8548a285094685f0bdc999e202d666b59eeb3df2ff921ab68b8336a49",
        file: "harfbuzz-11.2.1.tar.xz",
        licenses: &["COPYING"],
    },
    Source {
        name: "libass",
        version: "0.17.4",
        url: "https://github.com/libass/libass/releases/download/0.17.4/libass-0.17.4.tar.xz",
        sha256: "78f1179b838d025e9c26e8fef33f8092f65611444ffa1bfc0cfac6a33511a05a",
        file: "libass-0.17.4.tar.xz",
        licenses: &["COPYING"],
    },
    Source {
        name: "libplacebo",
        version: "7.349.0",
        url: "https://code.videolan.org/videolan/libplacebo/-/archive/v7.349.0/libplacebo-v7.349.0.tar.gz",
        sha256: "79120e685a1836344b51b13b6a5661622486a84e4d4a35f6c8d01679a20fbc86",
        file: "libplacebo-7.349.0.tar.gz",
        licenses: &["LICENSE"],
    },
    Source {
        name: "mpv",
        version: "0.40.0",
        url: "https://github.com/mpv-player/mpv/archive/refs/tags/v0.40.0.tar.gz",
        sha256: "10a0f4654f62140a6dd4d380dcf0bbdbdcf6e697556863dc499c296182f081a3",
        file: "mpv-0.40.0.tar.gz",
        licenses: &["LICENSE.LGPL", "Copyright"],
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
    fn mpv_and_ffmpeg_are_present_and_mpv_comes_last() {
        assert_eq!(source("ffmpeg").name, "ffmpeg");
        assert_eq!(SOURCES.last().unwrap().name, "mpv");
        assert_eq!(SOURCES.first().unwrap().name, "ffmpeg");
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
