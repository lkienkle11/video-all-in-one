//! What an ELF executable and its bundled libraries need from the system, read from the text of
//! `readelf -d`. The Ubuntu package derives its `Depends:` line from this, so the line lists
//! what the build really links and not a guess.

use anyhow::{bail, Result};
use std::collections::BTreeSet;

/// The oldest glibc the package promises to run on (Ubuntu 22.04).
pub(crate) const MINIMUM_GLIBC: &str = "2.35";

/// The `libc6` entry of every package that links against glibc.
fn libc_dependency() -> String {
    format!("libc6 (>= {MINIMUM_GLIBC})")
}

/// Sonames provided by glibc itself, all in the `libc6` package.
const GLIBC_SONAMES: [&str; 8] = [
    "libc.so.6",
    "libm.so.6",
    "libdl.so.2",
    "libpthread.so.0",
    "librt.so.1",
    "libresolv.so.2",
    "libutil.so.1",
    "ld-linux-x86-64.so.2",
];

/// Sonames of other system libraries and the Ubuntu package that ships each one.
const PACKAGES: [(&str, &str); 22] = [
    ("libgcc_s.so.1", "libgcc-s1"),
    ("libstdc++.so.6", "libstdc++6"),
    ("libz.so.1", "zlib1g"),
    ("libasound.so.2", "libasound2"),
    ("libpulse.so.0", "libpulse0"),
    ("libpulse-simple.so.0", "libpulse0"),
    ("libfontconfig.so.1", "libfontconfig1"),
    ("libfreetype.so.6", "libfreetype6"),
    ("libxcb.so.1", "libxcb1"),
    ("libxcb-xkb.so.1", "libxcb-xkb1"),
    ("libxkbcommon.so.0", "libxkbcommon0"),
    ("libxkbcommon-x11.so.0", "libxkbcommon-x11-0"),
    ("libX11.so.6", "libx11-6"),
    ("libX11-xcb.so.1", "libx11-xcb1"),
    ("libwayland-client.so.0", "libwayland-client0"),
    ("libwayland-cursor.so.0", "libwayland-cursor0"),
    ("libwayland-egl.so.1", "libwayland-egl1"),
    ("libvulkan.so.1", "libvulkan1"),
    ("libzstd.so.1", "libzstd1"),
    ("libexpat.so.1", "libexpat1"),
    ("libEGL.so.1", "libegl1"),
    ("libGL.so.1", "libgl1"),
];

/// The sonames listed as `(NEEDED)` in the output of `readelf -d`.
pub(crate) fn needed(readelf_d: &str) -> Vec<String> {
    bracketed(readelf_d, "(NEEDED)")
}

/// The run-path entries (`RUNPATH` or `RPATH`) in the output of `readelf -d`, split at colons.
pub(crate) fn run_paths(readelf_d: &str) -> Vec<String> {
    let mut found = bracketed(readelf_d, "(RUNPATH)");
    found.extend(bracketed(readelf_d, "(RPATH)"));
    found
        .iter()
        .flat_map(|list| list.split(':'))
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect()
}

/// The text inside `[...]` on each line that holds `tag`.
fn bracketed(text: &str, tag: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.contains(tag))
        .filter_map(|line| {
            let start = line.find('[')? + 1;
            let end = line.rfind(']')?;
            (end >= start).then(|| line[start..end].to_string())
        })
        .collect()
}

/// The `Depends:` entries a package needs: every soname in `needed` that is not one of the
/// `bundled` files, mapped to its Ubuntu package, sorted, `libc6` first. A soname this table
/// does not know is an error, so a new dependency is never left out of the package silently.
pub(crate) fn system_depends(needed: &[String], bundled: &[String]) -> Result<Vec<String>> {
    let mut packages = BTreeSet::new();
    let mut libc = false;
    let mut unknown = Vec::new();
    for soname in needed {
        if bundled.iter().any(|file| file == soname) {
            continue;
        }
        if GLIBC_SONAMES.contains(&soname.as_str()) {
            libc = true;
        } else if let Some((_, package)) = PACKAGES.iter().find(|(name, _)| name == soname) {
            packages.insert((*package).to_string());
        } else {
            unknown.push(soname.as_str());
        }
    }
    if !unknown.is_empty() {
        bail!(
            "no Ubuntu package is known for {}; add it to PACKAGES in xtask/src/package/elf.rs",
            unknown.join(", ")
        );
    }
    let mut depends = Vec::new();
    if libc {
        depends.push(libc_dependency());
    }
    depends.extend(packages);
    Ok(depends)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\nDynamic section at offset 0x2d8a0 contains 30 entries:\n  Tag        Type                         Name/Value\n 0x0000000000000001 (NEEDED)             Shared library: [libmpv.so.2]\n 0x0000000000000001 (NEEDED)             Shared library: [libasound.so.2]\n 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]\n 0x0000000000000001 (NEEDED)             Shared library: [libm.so.6]\n 0x000000000000001d (RUNPATH)            Library runpath: [$ORIGIN/../lib/video-all-in-one:/opt/x]\n 0x000000000000000c (INIT)               0x1000\n";

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn needed_and_run_paths_are_read_from_readelf_text() {
        assert_eq!(
            needed(SAMPLE),
            ["libmpv.so.2", "libasound.so.2", "libc.so.6", "libm.so.6"]
        );
        assert_eq!(
            run_paths(SAMPLE),
            ["$ORIGIN/../lib/video-all-in-one", "/opt/x"]
        );
        assert!(needed("no dynamic section").is_empty());
        assert!(run_paths("0x1 (RPATH)  Library rpath: []").is_empty());
    }

    #[test]
    fn bundled_libraries_are_not_system_dependencies() {
        let depends = system_depends(&needed(SAMPLE), &names(&["libmpv.so.2"])).unwrap();
        assert_eq!(depends, ["libc6 (>= 2.35)", "libasound2"]);
    }

    #[test]
    fn depends_are_sorted_unique_and_glibc_sonames_make_one_entry() {
        let list = names(&[
            "libpulse.so.0",
            "libpulse-simple.so.0",
            "libm.so.6",
            "libc.so.6",
            "libstdc++.so.6",
            "libz.so.1",
        ]);
        assert_eq!(
            system_depends(&list, &[]).unwrap(),
            ["libc6 (>= 2.35)", "libpulse0", "libstdc++6", "zlib1g"]
        );
        assert!(system_depends(&[], &[]).unwrap().is_empty());
    }

    #[test]
    fn an_unknown_soname_is_an_error_that_names_it() {
        let error = system_depends(&names(&["libmystery.so.9", "libc.so.6"]), &[])
            .unwrap_err()
            .to_string();
        assert!(error.contains("libmystery.so.9"), "{error}");
        assert!(!error.contains("libc.so.6"), "{error}");
    }
}
