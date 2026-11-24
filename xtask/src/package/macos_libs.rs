//! The dynamic libraries a macOS executable needs, found with `otool -L` and described as
//! pure data so the closure and the rewrite arguments can be tested without a Mac.

use anyhow::{bail, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// One non-system library to copy into `Contents/Frameworks`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Library {
    /// File name inside the bundle (the last component of the install name that referred to it).
    pub name: String,
    /// The real file to copy.
    pub source: PathBuf,
    /// Its non-system dependencies, as written in the library.
    pub deps: Vec<String>,
}

/// Everything an executable needs besides the operating system.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Closure {
    pub binary_deps: Vec<String>,
    pub libraries: Vec<Library>,
}

/// The install names listed by `otool -L`. The first line names the file; for a library the
/// next entry is its own install name, which is not a dependency.
pub(super) fn parse_dependencies(otool: &str, is_library: bool) -> Vec<String> {
    let entries = otool
        .lines()
        .skip(1)
        .filter_map(|line| line.strip_prefix('\t'))
        .map(|line| line.split(" (").next().unwrap_or(line).to_string());
    entries.skip(usize::from(is_library)).collect()
}

/// True for paths the operating system provides.
pub(super) fn is_system(path: &str) -> bool {
    path.starts_with("/usr/lib/") || path.starts_with("/System/Library/")
}

/// The file name a dependency gets inside the bundle.
pub(super) fn bundled_name(dep: &str) -> &str {
    dep.rsplit('/').next().unwrap_or(dep)
}

/// The install name every bundled library and the executable use for `name`.
pub(super) fn bundle_reference(name: &str) -> String {
    format!("@executable_path/../Frameworks/{name}")
}

/// Arguments of one `install_name_tool` call: set the id of a library and point each
/// dependency into the bundle.
pub(super) fn install_name_args(id: Option<&str>, deps: &[String], file: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();
    if let Some(name) = id {
        args.push("-id".into());
        args.push(bundle_reference(name).into());
    }
    for dep in deps {
        args.push("-change".into());
        args.push(dep.into());
        args.push(bundle_reference(bundled_name(dep)).into());
    }
    args.push(file.into());
    args
}

/// The absolute run-path entries (`LC_RPATH`) in the output of `otool -l`. A bundled library
/// reaches its dependencies through `@executable_path`, so these only point outside the bundle.
pub(super) fn absolute_rpaths(otool_l: &str) -> Vec<String> {
    let mut in_rpath = false;
    let mut found = Vec::new();
    for line in otool_l.lines().map(str::trim) {
        if let Some(command) = line.strip_prefix("cmd ") {
            in_rpath = command == "LC_RPATH";
        } else if let Some(rest) = line.strip_prefix("path ").filter(|_| in_rpath) {
            let path = rest.split(" (offset").next().unwrap_or(rest);
            if path.starts_with('/') {
                found.push(path.to_string());
            }
            in_rpath = false;
        }
    }
    found
}

/// The `minos` of the `LC_BUILD_VERSION` command in the output of `otool -l`.
pub(super) fn minimum_os(otool_l: &str) -> Option<String> {
    let mut key: Option<&str> = None;
    for line in otool_l.lines().map(str::trim) {
        if let Some(command) = line.strip_prefix("cmd ") {
            key = match command {
                "LC_BUILD_VERSION" => Some("minos "),
                "LC_VERSION_MIN_MACOSX" => Some("version "),
                _ => None,
            };
        } else if let Some(value) = key.and_then(|key| line.strip_prefix(key)) {
            return Some(value.trim().to_string());
        }
    }
    None
}

/// The highest of `versions` (dotted numbers such as `13.0` or `15.1.2`), or `floor` when none
/// is higher.
pub(super) fn highest_version(floor: &str, versions: &[String]) -> String {
    let number = |text: &str| -> Vec<u32> {
        text.split('.')
            .map(|part| part.parse().unwrap_or(0))
            .collect()
    };
    let mut best = floor;
    for version in versions {
        if number(version) > number(best) {
            best = version;
        }
    }
    best.to_string()
}

/// Where a dependency may live, in search order; `Err` for forms that cannot be bundled.
pub(super) fn candidate_paths(dep: &str, referrer_dir: &Path) -> Result<Vec<PathBuf>> {
    if let Some(rest) = dep.strip_prefix("@loader_path/") {
        return Ok(vec![referrer_dir.join(rest)]);
    }
    if let Some(rest) = dep.strip_prefix("@rpath/") {
        let dirs = [
            referrer_dir,
            Path::new("/opt/homebrew/lib"),
            Path::new("/usr/local/lib"),
        ];
        return Ok(dirs.iter().map(|dir| dir.join(rest)).collect());
    }
    if dep.starts_with('/') {
        return Ok(vec![PathBuf::from(dep)]);
    }
    bail!("cannot bundle {dep}: unsupported install name")
}

/// The real file behind `dep` on disk.
pub(super) fn resolve_on_disk(dep: &str, referrer_dir: &Path) -> Result<PathBuf> {
    for candidate in candidate_paths(dep, referrer_dir)? {
        if let Ok(real) = candidate.canonicalize() {
            return Ok(real);
        }
    }
    bail!("cannot find {dep}")
}

/// Walks the dependencies of `binary` through `list` (the output of `otool -L` for a path)
/// and `resolve`. Every library appears once; two different files that would share a bundle
/// name are an error.
pub(super) fn closure(
    binary: &Path,
    list: &mut dyn FnMut(&Path, bool) -> Result<String>,
    resolve: &dyn Fn(&str, &Path) -> Result<PathBuf>,
) -> Result<Closure> {
    let binary_deps: Vec<String> = parse_dependencies(&list(binary, false)?, false)
        .into_iter()
        .filter(|dep| !is_system(dep))
        .collect();
    let mut libraries: Vec<Library> = Vec::new();
    let mut queue: Vec<(String, PathBuf)> = binary_deps
        .iter()
        .map(|dep| (dep.clone(), PathBuf::new()))
        .collect();
    queue.reverse();
    while let Some((dep, referrer_dir)) = queue.pop() {
        let source = resolve(&dep, &referrer_dir)?;
        let name = bundled_name(&dep).to_string();
        if let Some(existing) = libraries.iter().find(|library| library.name == name) {
            if existing.source != source {
                bail!(
                    "{name} would be bundled from two files: {} and {}",
                    existing.source.display(),
                    source.display()
                );
            }
            continue;
        }
        let deps: Vec<String> = parse_dependencies(&list(&source, true)?, true)
            .into_iter()
            .filter(|dep| !is_system(dep))
            .collect();
        let dir = source.parent().map(Path::to_path_buf).unwrap_or_default();
        for next in deps.iter().rev() {
            queue.push((next.clone(), dir.clone()));
        }
        libraries.push(Library { name, source, deps });
    }
    Ok(Closure {
        binary_deps,
        libraries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const BINARY_OTOOL: &str = "app:\n\t/opt/homebrew/opt/mpv/lib/libmpv.2.dylib (compatibility version 2.0.0, current version 2.0.0)\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1356.0.0)\n";
    const LIB_OTOOL: &str = "/opt/a/libmpv.2.dylib:\n\t/opt/homebrew/opt/mpv/lib/libmpv.2.dylib (compatibility version 2.0.0, current version 2.0.0)\n\t/opt/b/libb.1.dylib (compatibility version 1.0.0)\n\t/System/Library/Frameworks/AppKit.framework/Versions/C/AppKit (compatibility version 45.0.0)\n";

    #[test]
    fn dependencies_skip_the_file_line_and_the_library_id() {
        assert_eq!(
            parse_dependencies(BINARY_OTOOL, false),
            [
                "/opt/homebrew/opt/mpv/lib/libmpv.2.dylib",
                "/usr/lib/libSystem.B.dylib"
            ]
        );
        assert_eq!(
            parse_dependencies(LIB_OTOOL, true),
            [
                "/opt/b/libb.1.dylib",
                "/System/Library/Frameworks/AppKit.framework/Versions/C/AppKit"
            ]
        );
    }

    #[test]
    fn system_paths_are_not_bundled() {
        for path in [
            "/usr/lib/libz.1.dylib",
            "/System/Library/Frameworks/Metal.framework/Metal",
            "/usr/lib/swift/libswiftCore.dylib",
        ] {
            assert!(is_system(path), "{path}");
        }
        for path in [
            "/opt/homebrew/opt/ffmpeg/lib/libavcodec.63.dylib",
            "/usr/local/lib/libx.dylib",
            "@rpath/libx.dylib",
            "/usr/libx/evil.dylib",
        ] {
            assert!(!is_system(path), "{path}");
        }
    }

    #[test]
    fn bundle_references_point_into_frameworks() {
        assert_eq!(
            bundled_name("/opt/homebrew/opt/mpv/lib/libmpv.2.dylib"),
            "libmpv.2.dylib"
        );
        assert_eq!(
            bundle_reference("libmpv.2.dylib"),
            "@executable_path/../Frameworks/libmpv.2.dylib"
        );
        let args = install_name_args(
            Some("libb.1.dylib"),
            &["/opt/b/libc.dylib".to_string()],
            Path::new("/stage/libb.1.dylib"),
        );
        let text: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            text,
            [
                "-id",
                "@executable_path/../Frameworks/libb.1.dylib",
                "-change",
                "/opt/b/libc.dylib",
                "@executable_path/../Frameworks/libc.dylib",
                "/stage/libb.1.dylib"
            ]
        );
        let binary = install_name_args(None, &["/x/liba.dylib".to_string()], Path::new("app"));
        assert_eq!(binary.len(), 4, "an executable has no id to set");
    }

    #[test]
    fn absolute_rpaths_are_found_and_relative_ones_kept() {
        let text = "Load command 9\n          cmd LC_RPATH\n      cmdsize 48\n         path /opt/homebrew/Cellar/jpeg-turbo/3.2.1/lib (offset 12)\nLoad command 10\n          cmd LC_RPATH\n      cmdsize 32\n         path @loader_path/../lib (offset 12)\nLoad command 11\n          cmd LC_LOAD_DYLIB\n         name /usr/lib/libz.1.dylib (offset 24)\n         path /not/an/rpath (offset 1)\n";
        assert_eq!(
            absolute_rpaths(text),
            ["/opt/homebrew/Cellar/jpeg-turbo/3.2.1/lib"]
        );
    }

    #[test]
    fn minimum_os_is_read_and_the_highest_wins() {
        let text = "Load command 3\n      cmd LC_BUILD_VERSION\n  cmdsize 32\n platform 1\n    minos 15.0\n      sdk 26.5\n   ntools 1\n";
        assert_eq!(minimum_os(text).as_deref(), Some("15.0"));
        assert_eq!(minimum_os("cmd LC_ID_DYLIB\n    minos 99.0\n"), None);
        let versions = ["11.0".to_string(), "15.0".to_string(), "9.9".to_string()];
        assert_eq!(highest_version("13.0", &versions), "15.0");
        assert_eq!(highest_version("13.0", &["10.15".to_string()]), "13.0");
        assert_eq!(
            highest_version("13.0", &["14.4.1".to_string(), "14.10".to_string()]),
            "14.10"
        );
        assert_eq!(highest_version("13.0", &[]), "13.0");
    }

    #[test]
    fn candidates_follow_the_install_name_form() {
        let dir = Path::new("/stage/lib");
        assert_eq!(
            candidate_paths("/opt/x/liby.dylib", dir).unwrap(),
            [PathBuf::from("/opt/x/liby.dylib")]
        );
        assert_eq!(
            candidate_paths("@loader_path/../z.dylib", dir).unwrap(),
            [PathBuf::from("/stage/lib/../z.dylib")]
        );
        let rpath = candidate_paths("@rpath/libr.dylib", dir).unwrap();
        assert_eq!(rpath[0], PathBuf::from("/stage/lib/libr.dylib"));
        assert!(rpath.contains(&PathBuf::from("/opt/homebrew/lib/libr.dylib")));
        assert!(candidate_paths("@executable_path/x.dylib", dir).is_err());
        assert!(candidate_paths("relative/x.dylib", dir).is_err());
    }

    fn fake(files: &[(&str, &str)]) -> BTreeMap<String, String> {
        files
            .iter()
            .map(|(p, o)| (p.to_string(), o.to_string()))
            .collect()
    }

    fn walk(files: &BTreeMap<String, String>) -> Result<Closure> {
        let mut list = |path: &Path, _lib: bool| -> Result<String> {
            files
                .get(path.to_str().unwrap())
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no {path:?}"))
        };
        closure(Path::new("app"), &mut list, &|dep, _| {
            Ok(PathBuf::from(dep))
        })
    }

    #[test]
    fn closure_visits_each_library_once_even_in_a_cycle() {
        let files = fake(&[
            ("app", "app:\n\t/opt/a/liba.dylib (x)\n\t/usr/lib/libSystem.B.dylib (x)\n"),
            ("/opt/a/liba.dylib", "x:\n\t/opt/a/liba.dylib (x)\n\t/opt/b/libb.dylib (x)\n"),
            ("/opt/b/libb.dylib", "x:\n\t/opt/b/libb.dylib (x)\n\t/opt/a/liba.dylib (x)\n\t/usr/lib/libz.1.dylib (x)\n"),
        ]);
        let found = walk(&files).unwrap();
        assert_eq!(found.binary_deps, ["/opt/a/liba.dylib"]);
        let names: Vec<&str> = found.libraries.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["liba.dylib", "libb.dylib"]);
        assert_eq!(found.libraries[1].deps, ["/opt/a/liba.dylib"]);
    }

    #[test]
    fn two_files_with_one_name_are_an_error() {
        let files = fake(&[
            (
                "app",
                "app:\n\t/opt/a/libx.dylib (x)\n\t/opt/b/libx.dylib (x)\n",
            ),
            ("/opt/a/libx.dylib", "x:\n\t/opt/a/libx.dylib (x)\n"),
            ("/opt/b/libx.dylib", "x:\n\t/opt/b/libx.dylib (x)\n"),
        ]);
        let message = format!("{:#}", walk(&files).unwrap_err());
        assert!(message.contains("libx.dylib"), "{message}");
    }
}
