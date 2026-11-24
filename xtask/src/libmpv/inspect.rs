//! Reading the output of the tools that inspect a finished build: `otool -L`, and the probe
//! program that prints `mpv_client_api_version()`. Pure text functions, tested on any machine.

use std::path::Path;

/// The dependencies in `otool -L` text that would not be found after the folder is moved: absolute
/// paths outside the operating system's own folders. `@rpath/`, `@loader_path/` and
/// `@executable_path/` references are relocatable. The first entry of a library is its own id and
/// is skipped.
pub(super) fn macho_absolute_references(otool_l: &str, is_library: bool) -> Vec<String> {
    otool_l
        .lines()
        .skip(1)
        .filter_map(|line| line.strip_prefix('\t'))
        .map(|line| line.split(" (").next().unwrap_or(line).to_string())
        .skip(usize::from(is_library))
        .filter(|path| {
            !(path.starts_with("@")
                || path.starts_with("/usr/lib/")
                || path.starts_with("/System/Library/"))
        })
        .collect()
}

/// The install name (first entry) of a library in `otool -L` text.
pub(super) fn macho_install_name(otool_l: &str) -> Option<String> {
    otool_l
        .lines()
        .nth(1)?
        .strip_prefix('\t')
        .map(|line| line.split(" (").next().unwrap_or(line).to_string())
}

/// Every run-path entry (`LC_RPATH`) in the output of `otool -l`.
pub(super) fn macho_rpaths(otool_l: &str) -> Vec<String> {
    let mut in_rpath = false;
    let mut found = Vec::new();
    for line in otool_l.lines().map(str::trim) {
        if let Some(command) = line.strip_prefix("cmd ") {
            in_rpath = command == "LC_RPATH";
        } else if let Some(rest) = line.strip_prefix("path ").filter(|_| in_rpath) {
            found.push(rest.split(" (offset").next().unwrap_or(rest).to_string());
            in_rpath = false;
        }
    }
    found
}

/// The major version in the number `mpv_client_api_version()` returns (`major << 16 | minor`),
/// from the single line the probe program prints.
pub(super) fn api_major(probe_output: &str) -> Option<u32> {
    let value: u32 = probe_output.trim().parse().ok()?;
    Some(value >> 16)
}

/// The C program that prints the client API version of the libmpv it is linked with.
pub(super) const PROBE_SOURCE: &str = "#include <stdio.h>\n#include <mpv/client.h>\nint main(void) { printf(\"%lu\\n\", mpv_client_api_version()); return 0; }\n";

/// The reference of `file` that a dependency written as `from` must have after `prefix` moved:
/// `@rpath/<file name>`.
pub(super) fn rpath_reference(from: &str) -> Option<String> {
    let name = Path::new(from).file_name()?.to_str()?;
    Some(format!("@rpath/{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIB: &str = "/o/lib/libmpv.2.dylib:\n\t@rpath/libmpv.2.dylib (compatibility version 2.0.0, current version 2.5.0)\n\t@rpath/libavcodec.61.dylib (compatibility version 61.0.0)\n\t/o/lib/libplacebo.349.dylib (compatibility version 349.0.0)\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0)\n\t/System/Library/Frameworks/CoreAudio.framework/Versions/A/CoreAudio (compatibility version 1.0.0)\n\t/opt/homebrew/lib/libz.dylib (compatibility version 1.0.0)\n";

    #[test]
    fn absolute_references_outside_the_system_are_found() {
        assert_eq!(
            macho_absolute_references(LIB, true),
            [
                "/o/lib/libplacebo.349.dylib",
                "/opt/homebrew/lib/libz.dylib"
            ]
        );
        // For an executable the first entry is a dependency, not an id.
        assert_eq!(
            macho_absolute_references("app:\n\t/o/lib/libmpv.dylib (x)\n", false),
            ["/o/lib/libmpv.dylib"]
        );
        assert!(macho_absolute_references("x:\n\t@rpath/libx.dylib (x)\n", true).is_empty());
    }

    #[test]
    fn install_name_is_the_first_entry() {
        assert_eq!(
            macho_install_name(LIB).as_deref(),
            Some("@rpath/libmpv.2.dylib")
        );
        assert_eq!(macho_install_name("only a file line"), None);
    }

    #[test]
    fn api_major_reads_the_probe_output() {
        // mpv 0.40 reports client API 2.5; Ubuntu 22.04's libmpv1 reports 1.109.
        assert_eq!(api_major("131077\n"), Some(2));
        assert_eq!(api_major("65645"), Some(1));
        assert_eq!(api_major("not a number"), None);
        assert_eq!(api_major(""), None);
    }

    #[test]
    fn rpath_references_use_the_file_name_only() {
        assert_eq!(
            rpath_reference("/o/lib/libass.9.dylib").as_deref(),
            Some("@rpath/libass.9.dylib")
        );
        assert_eq!(rpath_reference(""), None);
    }

    #[test]
    fn every_run_path_is_listed_whatever_its_form() {
        let text = "Load command 9\n          cmd LC_RPATH\n      cmdsize 48\n         path /o/lib (offset 12)\nLoad command 10\n          cmd LC_RPATH\n         path @loader_path (offset 12)\nLoad command 11\n          cmd LC_LOAD_DYLIB\n         path /not/one (offset 1)\n";
        assert_eq!(macho_rpaths(text), ["/o/lib", "@loader_path"]);
        assert!(macho_rpaths("cmd LC_ID_DYLIB\n").is_empty());
    }

    #[test]
    fn the_probe_calls_the_version_function() {
        assert!(PROBE_SOURCE.contains("mpv_client_api_version()"));
    }
}
