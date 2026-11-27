//! The license gate of the packagers: release packages may bundle only LGPL builds of libmpv
//! and FFmpeg. An FFmpeg library records its configure line, so a GPL or non-free build can be
//! recognized by `--enable-gpl`, `--enable-nonfree` or `--enable-libx264` in its bytes;
//! the same scan covers bundled FFmpeg and FFprobe executables. The GPL codec libraries are
//! recognized by their file names. libmpv has no such marker, so it is not checked here.

use anyhow::{bail, Context, Result};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The explicit flag of `cargo xtask package` that lets a GPL build through (never in releases).
pub(crate) const ALLOW_GPL_FLAG: &str = "--allow-gpl-for-local-testing";

/// Configure options that make an FFmpeg build GPL or non-free, with what each one means.
const FORBIDDEN_FLAGS: [(&str, &str); 3] = [
    ("--enable-gpl", "a GPL build of FFmpeg"),
    ("--enable-nonfree", "a non-free build of FFmpeg"),
    ("--enable-libx264", "a GPL libx264 encoder in FFmpeg"),
];

/// Requires both tools and scans their embedded configuration without executing foreign code.
pub(super) fn bundled_tools(from: &Path, windows: bool, allow_gpl: bool) -> Result<[PathBuf; 2]> {
    let names = if windows {
        ["ffmpeg.exe", "ffprobe.exe"]
    } else {
        ["ffmpeg", "ffprobe"]
    };
    let tools = names.map(|name| from.join(name));
    for tool in &tools {
        if !tool.is_file() {
            bail!("required bundled tool {} is missing; provide FFmpeg and FFprobe from the LGPL build", tool.display());
        }
    }
    for warning in check_bundle_license(&tools, allow_gpl)? {
        println!("warning: {warning}");
    }
    Ok(tools)
}

/// File name prefixes of GPL or non-free codec libraries.
const GPL_LIBRARY_PREFIXES: [&str; 5] =
    ["libx264", "libx265", "libxvid", "libfdk-aac", "libpostproc"];

/// How many bytes `check_bundle_license` reads at a time.
const CHUNK_SIZE: usize = 1 << 20;

/// Checks every bundled library and returns the warnings. A library that is GPL or non-free is an
/// error, unless `allow_gpl` is set (local testing): then it becomes a warning.
pub(crate) fn check_bundle_license(files: &[PathBuf], allow_gpl: bool) -> Result<Vec<String>> {
    let mut problems = Vec::new();
    for file in files {
        problems.extend(problems_of(file, CHUNK_SIZE)?);
    }
    if problems.is_empty() {
        return Ok(Vec::new());
    }
    if !allow_gpl {
        bail!(
            "refusing to package: release packages may bundle only LGPL libraries.\n  {}\nBuild libmpv and FFmpeg as LGPL (see docs/RELEASING.md).",
            problems.join("\n  ")
        );
    }
    let mut warnings = problems;
    warnings.push(format!(
        "{ALLOW_GPL_FLAG} was used: this package contains GPL or non-free libraries and must not be published"
    ));
    Ok(warnings)
}

/// What is wrong with one library: a GPL codec name, or a forbidden configure option inside.
fn problems_of(file: &Path, chunk_size: usize) -> Result<Vec<String>> {
    let mut problems = Vec::new();
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    if GPL_LIBRARY_PREFIXES.iter().any(|p| lower.starts_with(p)) {
        problems.push(format!(
            "{} is a GPL or non-free codec library ({})",
            name,
            file.display()
        ));
    }
    let needles: Vec<&[u8]> = FORBIDDEN_FLAGS
        .iter()
        .map(|(flag, _)| flag.as_bytes())
        .collect();
    let reader = File::open(file).with_context(|| format!("opening {}", file.display()))?;
    let found = scan_reader(reader, &needles, chunk_size)
        .with_context(|| format!("reading {}", file.display()))?;
    for index in found {
        let (flag, meaning) = FORBIDDEN_FLAGS[index];
        problems.push(format!(
            "{} contains `{flag}`: {meaning} ({})",
            name,
            file.display()
        ));
    }
    Ok(problems)
}

/// The indexes of the `needles` that occur in `reader`. Reads `chunk_size` bytes at a time and
/// keeps the tail of the previous chunk, so a needle that straddles two chunks is still found.
fn scan_reader<R: Read>(
    mut reader: R,
    needles: &[&[u8]],
    chunk_size: usize,
) -> std::io::Result<Vec<usize>> {
    let keep = needles.iter().map(|n| n.len()).max().unwrap_or(1) - 1;
    let mut found = vec![false; needles.len()];
    let mut window: Vec<u8> = Vec::with_capacity(chunk_size + keep);
    let mut chunk = vec![0u8; chunk_size.max(1)];
    loop {
        let read = match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        window.extend_from_slice(&chunk[..read]);
        for (index, needle) in needles.iter().enumerate() {
            if !found[index] && contains(&window, needle) {
                found[index] = true;
            }
        }
        let drop = window.len().saturating_sub(keep);
        window.drain(..drop);
    }
    Ok(found
        .iter()
        .enumerate()
        .filter_map(|(index, hit)| hit.then_some(index))
        .collect())
}

/// Whether `needle` occurs in `haystack`; looks only where the first byte matches.
pub(crate) fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    let Some((&first, _)) = needle.split_first() else {
        return true;
    };
    let mut start = 0;
    while let Some(at) = haystack[start..].iter().position(|&b| b == first) {
        let from = start + at;
        if haystack[from..].starts_with(needle) {
            return true;
        }
        start = from + 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "vaio-license-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn library(contents: &str) -> Vec<u8> {
        let mut bytes = vec![0u8, 1, 2, 255];
        bytes.extend_from_slice(contents.as_bytes());
        bytes.extend_from_slice(&[0, 9, 8]);
        bytes
    }

    #[test]
    fn package_whose_ffmpeg_lists_libx264_is_refused() {
        let dir = TempDir::new();
        let file = dir.file("ffmpeg", &library("--enable-libx264"));
        let error = check_bundle_license(&[file], false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("--enable-libx264"), "{error}");
    }

    #[test]
    fn package_without_tools_is_refused() {
        let dir = TempDir::new();
        let binary = dir.file("app", b"fixture");
        for missing in ["ffmpeg.exe", "ffprobe.exe"] {
            let libs = dir.0.join(missing);
            std::fs::create_dir_all(&libs).unwrap();
            let present = if missing == "ffmpeg.exe" {
                "ffprobe.exe"
            } else {
                "ffmpeg.exe"
            };
            std::fs::write(libs.join(present), b"fixture").unwrap();
            let error = super::super::nsis::build(
                &dir.0,
                &binary,
                &dir.0.join("out"),
                "1.2.3",
                "windows-x86_64",
                Some(&libs),
                false,
            )
            .unwrap_err()
            .to_string();
            assert!(
                error.contains("required bundled tool") && error.contains(missing),
                "missing {missing}: {error}"
            );
        }
    }

    #[test]
    fn gpl_ffmpeg_exe_is_refused() {
        let dir = super::super::tests::ToolFixture::new();
        std::fs::write(dir.0.join("ffmpeg.exe"), library("--enable-gpl")).unwrap();
        let error = super::super::nsis::build(
            &dir.0,
            &dir.0.join("app"),
            &dir.0.join("out"),
            "1.2.3",
            "windows-x86_64",
            Some(&dir.0),
            false,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("--enable-gpl"), "{error}");
    }

    #[test]
    fn gpl_flag_in_a_library_is_refused() {
        let dir = TempDir::new();
        let file = dir.file(
            "libavcodec.61.dylib",
            &library("--enable-shared --enable-gpl --enable-libx264"),
        );
        let error = check_bundle_license(&[file], false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("libavcodec.61.dylib"), "{error}");
        assert!(error.contains("--enable-gpl"), "{error}");
    }

    #[test]
    fn nonfree_flag_in_a_library_is_refused() {
        let dir = TempDir::new();
        let file = dir.file(
            "avcodec-61.dll",
            &library("--enable-nonfree --enable-shared"),
        );
        let error = check_bundle_license(&[file], false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("--enable-nonfree"), "{error}");
    }

    #[test]
    fn gpl_codec_library_names_are_refused() {
        let dir = TempDir::new();
        for name in [
            "libx264.164.dylib",
            "libx265.215.dylib",
            "libxvidcore.4.dylib",
            "libfdk-aac.2.dylib",
            "libpostproc.58.dylib",
            "LIBX264-164.dll",
        ] {
            let file = dir.file(name, b"harmless bytes");
            let error = check_bundle_license(&[file], false)
                .unwrap_err()
                .to_string();
            assert!(error.contains(name), "{error}");
        }
    }

    #[test]
    fn lgpl_configuration_is_accepted() {
        let dir = TempDir::new();
        let file = dir.file(
            "libavcodec.61.dylib",
            &library("--prefix=/x --enable-shared --enable-version3 --disable-gpl --enable-pic"),
        );
        assert_eq!(
            check_bundle_license(&[file], false).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn override_flag_allows_gpl_but_reports_it() {
        let dir = TempDir::new();
        let file = dir.file("libavcodec.61.dylib", &library("--enable-gpl"));
        let warnings = check_bundle_license(&[file], true).unwrap();
        assert!(warnings.iter().any(|w| w.contains("--enable-gpl")));
        assert!(warnings.iter().any(|w| w.contains("must not be published")));
    }

    #[test]
    fn scan_reads_files_in_chunks_and_finds_a_flag_that_straddles_a_chunk_boundary() {
        let needle = b"--enable-gpl";
        let needles: [&[u8]; 1] = [needle];
        for chunk in [1usize, 2, 3, 5, 7, 11, 12, 13, 100] {
            for offset in 0..30 {
                let mut bytes = vec![b'a'; offset];
                bytes.extend_from_slice(needle);
                bytes.extend_from_slice(&[b'b'; 9]);
                let found = scan_reader(bytes.as_slice(), &needles, chunk).unwrap();
                assert_eq!(found, vec![0], "chunk {chunk} offset {offset}");
            }
        }
        let none = scan_reader(&b"--enable-gp --enable-gp"[..], &needles, 4).unwrap();
        assert!(none.is_empty());
        let dir = TempDir::new();
        let mut bytes = vec![b'x'; CHUNK_SIZE - 5];
        bytes.extend_from_slice(needle);
        let file = dir.file("libavutil.dylib", &bytes);
        assert!(!problems_of(&file, CHUNK_SIZE).unwrap().is_empty());
    }

    #[test]
    fn no_workflow_passes_the_gpl_override_flag() {
        let workflows = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows");
        let mut checked = 0;
        for entry in std::fs::read_dir(&workflows).unwrap() {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(!text.contains(ALLOW_GPL_FLAG), "{} uses it", path.display());
            checked += 1;
        }
        assert!(checked > 0);
    }

    /// The workflows that use our own libmpv pin its hash in `LIBMPV_SHA256_*` variables. The
    /// text `PENDING-first-build` may stand only there (until the first build is published), and
    /// the system libmpv packages must not come back.
    #[test]
    fn pending_hash_text_stands_only_in_the_pinned_variables_and_system_libmpv_is_gone() {
        let workflows = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows");
        for name in ["ci.yml", "release.yml"] {
            let text = std::fs::read_to_string(workflows.join(name)).unwrap();
            let mut pinned = 0;
            for line in text.lines() {
                if line.contains("PENDING-first-build") {
                    let variable = line.trim_start().starts_with("LIBMPV_SHA256_")
                        && !line.trim_start().starts_with('#');
                    assert!(
                        variable,
                        "{name}: PENDING-first-build outside a pinned hash: {line}"
                    );
                    pinned += 1;
                }
            }
            assert!(
                pinned == 0 || pinned == 3,
                "{name}: all three hashes or none ({pinned})"
            );
            for variable in ["LINUX_X86_64", "MACOS_ARM64", "MACOS_X86_64"] {
                assert!(
                    text.contains(&format!("LIBMPV_SHA256_{variable}:")),
                    "{name} {variable}"
                );
            }
            assert!(
                !text.contains("libmpv-dev"),
                "{name} installs the system libmpv"
            );
            assert!(
                !text.contains("brew install mpv"),
                "{name} installs Homebrew's libmpv"
            );
            assert!(
                text.contains("ubuntu-22.04"),
                "{name} must build on Ubuntu 22.04"
            );
        }
    }
}
