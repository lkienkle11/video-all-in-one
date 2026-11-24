//! The text files of the package: `MANIFEST.txt` (what was built from what, with which flags)
//! and `LICENSES/SOURCES.txt` (where the source of every component can be had, as the LGPL
//! requires).

use super::sources::Source;
use super::Target;

/// One built component and the flags it was configured with.
pub(super) struct Built<'a> {
    pub source: &'a Source,
    pub flags: Vec<String>,
}

/// `MANIFEST.txt`: a header and one line per component, `name | version | source sha256 | flags`.
pub(super) fn manifest(recipe: &str, target: Target, built: &[Built]) -> String {
    let mut text = format!("libmpv recipe {recipe} for {target:?}\n");
    text.push_str("component | version | sha256 of the source archive | configure flags\n");
    for item in built {
        text.push_str(&format!(
            "{} | {} | {} | {}\n",
            item.source.name,
            item.source.version,
            item.source.sha256,
            item.flags.join(" ")
        ));
    }
    text
}

/// `LICENSES/SOURCES.txt`: the download URL of the source archive of every component.
pub(super) fn sources_notice(built: &[Built]) -> String {
    let mut text = String::from(
        "The libraries in this package are built, unmodified, from these source archives.\n\
         The licence texts of each component are next to this file.\n\n",
    );
    for item in built {
        text.push_str(&format!(
            "{} {}\n  {}\n  sha256 {}\n",
            item.source.name, item.source.version, item.source.url, item.source.sha256
        ));
    }
    text
}

/// The `.pc` text with its absolute `prefix=` replaced by a path relative to the file, so that
/// the folder can be unpacked anywhere. `prefix` is the build prefix.
pub(super) fn relocate_pkgconfig(text: &str, prefix: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.starts_with("prefix=") {
            out.push("prefix=${pcfiledir}/../..".to_string());
        } else {
            out.push(line.replace(prefix, "${prefix}"));
        }
    }
    out.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libmpv::sources::source;

    fn built() -> Vec<Built<'static>> {
        vec![
            Built {
                source: source("ffmpeg"),
                flags: vec!["--disable-gpl".into(), "--enable-shared".into()],
            },
            Built {
                source: source("mpv"),
                flags: vec!["-Dgpl=false".into()],
            },
        ]
    }

    #[test]
    fn manifest_lists_component_version_hash_and_flags() {
        let text = manifest("1", Target::Ubuntu, &built());
        assert!(text.starts_with("libmpv recipe 1 for Ubuntu\n"));
        let line = text.lines().find(|l| l.starts_with("ffmpeg |")).unwrap();
        assert_eq!(
            line,
            format!(
                "ffmpeg | 7.1.5 | {} | --disable-gpl --enable-shared",
                source("ffmpeg").sha256
            )
        );
        assert!(text.contains("mpv | 0.40.0 |"));
        assert!(text.contains("| -Dgpl=false\n"));
    }

    #[test]
    fn sources_notice_has_every_url_and_hash() {
        let text = sources_notice(&built());
        for item in built() {
            assert!(text.contains(item.source.url), "{}", item.source.name);
            assert!(text.contains(item.source.sha256));
        }
    }

    #[test]
    fn pkgconfig_prefix_becomes_relative_to_the_file() {
        let pc = "prefix=/build/out\nlibdir=${prefix}/lib\nincludedir=/build/out/include\nName: x\nLibs: -L${libdir} -lx\n";
        let fixed = relocate_pkgconfig(pc, "/build/out");
        assert!(fixed.starts_with("prefix=${pcfiledir}/../..\n"));
        assert!(fixed.contains("includedir=${prefix}/include\n"));
        assert!(!fixed.contains("/build/out"));
        assert!(fixed.contains("libdir=${prefix}/lib\n"));
    }
}
