//! Running the build: the environment, the tools it needs, and each component's configure,
//! compile and install steps. Everything is installed into one prefix, the output folder.

use super::fetch;
use super::manifest::Built;
use super::recipe;
use super::sources::{Source, RECIPE_VERSION, SOURCES};
use super::Target;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The folder (inside the output folder) that holds one marker file per installed component.
pub(super) const MARKERS_DIR: &str = ".markers";

/// How a component is built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// `configure`, `make`, `make install`.
    Autotools,
    /// `meson setup`, `meson compile`, `meson install`.
    Meson,
}

/// Every component in build order with its build system and its arguments.
pub(super) fn plan(target: Target, prefix: &str) -> Vec<(&'static Source, Kind, Vec<String>)> {
    SOURCES
        .iter()
        .filter(|source| source.nested.is_none())
        .map(|source| {
            let (kind, args) = match source.name {
                "ffmpeg" => (Kind::Autotools, recipe::ffmpeg_configure(target, prefix)),
                "freetype" => (Kind::Autotools, recipe::freetype_configure(prefix)),
                "fribidi" => (Kind::Autotools, recipe::fribidi_configure(prefix)),
                "harfbuzz" => (Kind::Meson, recipe::harfbuzz_meson(prefix)),
                "libass" => (Kind::Autotools, recipe::libass_configure(target, prefix)),
                "libplacebo" => (Kind::Meson, recipe::libplacebo_meson(prefix)),
                "mpv" => (Kind::Meson, recipe::mpv_meson(target, prefix)),
                other => panic!("no build recipe for {other}"),
            };
            (source, kind, args)
        })
        .collect()
}

/// The programs the build starts, for a build on a machine of CPU `arch`.
pub(super) fn required_tools(target: Target, arch: &str) -> Vec<&'static str> {
    let mut tools = vec!["curl", "tar", "make", "cc", "pkg-config", "meson", "ninja"];
    if arch == "x86_64" {
        tools.push("nasm");
    }
    match target {
        Target::Ubuntu => tools.extend(["patchelf", "readelf"]),
        Target::Macos => tools.extend(["install_name_tool", "otool", "codesign"]),
    }
    tools
}

/// The tools of `required_tools` that cannot be started.
pub(super) fn missing_tools(target: Target) -> Vec<&'static str> {
    required_tools(target, std::env::consts::ARCH)
        .into_iter()
        .filter(|tool| Command::new(tool).arg("--version").output().is_err())
        .collect()
}

/// The only environment the build tools see: a few variables copied from `inherited`, and the
/// ones that point the tools at `prefix`. Nothing else leaks in (no `RUSTFLAGS`, `CFLAGS`,
/// `LIBRARY_PATH`, Homebrew `pkg-config` folders, ...).
pub(super) fn clean_env(
    target: Target,
    prefix: &Path,
    inherited: &dyn Fn(&str) -> Option<String>,
) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = Vec::new();
    for name in ["PATH", "HOME", "TMPDIR", "SDKROOT", "DEVELOPER_DIR"] {
        if let Some(value) = inherited(name) {
            env.push((name.to_string(), value));
        }
    }
    let lib = prefix.join("lib");
    let pkgconfig = lib.join("pkgconfig").display().to_string();
    env.push(("LC_ALL".into(), "C".into()));
    env.push(("PKG_CONFIG_PATH".into(), pkgconfig.clone()));
    match target {
        Target::Ubuntu => env.push(("LD_LIBRARY_PATH".into(), lib.display().to_string())),
        Target::Macos => {
            // Only the prefix: the Homebrew .pc files must not be found.
            env.push(("PKG_CONFIG_LIBDIR".into(), pkgconfig));
            env.push((
                "DYLD_FALLBACK_LIBRARY_PATH".into(),
                lib.display().to_string(),
            ));
            env.push((
                "MACOSX_DEPLOYMENT_TARGET".into(),
                super::MACOS_DEPLOYMENT_TARGET.into(),
            ));
        }
    }
    env
}

/// Runs `program` in `dir` with exactly `env`, and fails with its name when it does not succeed.
pub(super) fn run(
    program: &Path,
    args: &[String],
    dir: &Path,
    env: &[(String, String)],
) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .status()
        .with_context(|| format!("starting {}", program.display()))?;
    if !status.success() {
        bail!("{} failed with {status}", program.display());
    }
    Ok(())
}

/// The text that decides whether an installed component is still current: the recipe version,
/// the target, the build system, the pinned source (and the sources unpacked into it) and the
/// flags. Any change gives a different hash.
pub(super) fn recipe_hash(
    target: Target,
    source: &Source,
    kind: Kind,
    nested: &[&Source],
    args: &[String],
) -> String {
    let mut text = format!(
        "recipe {RECIPE_VERSION}\n{target:?} {kind:?}\n{} {} {}\n",
        source.name, source.version, source.sha256
    );
    for item in nested {
        text.push_str(&format!("nested {} {}\n", item.name, item.sha256));
    }
    text.push_str(&args.join("\n"));
    fetch::sha256_bytes(text.as_bytes())
}

/// Where the marker of `name` lives: a file holding the recipe hash it was installed with.
pub(super) fn marker_path(prefix: &Path, name: &str) -> PathBuf {
    prefix.join(MARKERS_DIR).join(name)
}

/// Downloads, verifies, unpacks, builds and installs every component into `prefix`. A component
/// whose marker holds the current recipe hash is already installed (from a cached folder) and
/// is not built again; its sources are still unpacked, because the licence texts come from them.
pub(super) fn build_all(
    target: Target,
    prefix: &Path,
    work: &Path,
    env: &[(String, String)],
) -> Result<Vec<Built<'static>>> {
    let jobs = std::thread::available_parallelism().map_or(2, |n| n.get());
    let download = work.join("download");
    let mut built = Vec::new();
    for (source, kind, args) in plan(target, &prefix.display().to_string()) {
        println!("== {} {}", source.name, source.version);
        let src = unpack(source, work, &download)?;
        let nested: Vec<&'static Source> = SOURCES
            .iter()
            .filter(|n| n.nested.is_some_and(|(parent, _)| parent == source.name))
            .collect();
        for item in &nested {
            unpack(item, work, &download)?;
        }
        let hash = recipe_hash(target, source, kind, &nested, &args);
        let marker = marker_path(prefix, source.name);
        if std::fs::read_to_string(&marker).is_ok_and(|text| text.trim() == hash) {
            println!(
                "{} is already installed (marker matches), skipping",
                source.name
            );
        } else {
            let build = work.join("build").join(source.name);
            if build.exists() {
                std::fs::remove_dir_all(&build)
                    .with_context(|| format!("clearing {}", build.display()))?;
            }
            std::fs::create_dir_all(&build)?;
            compile(source, kind, &args, &src, &build, jobs, env)?;
            std::fs::create_dir_all(prefix.join(MARKERS_DIR))?;
            std::fs::write(&marker, format!("{hash}\n"))?;
        }
        built.push(Built {
            source,
            flags: args,
        });
        for item in nested {
            let note = format!(
                "unpacked into {} (a git submodule of its release)",
                source.name
            );
            built.push(Built {
                source: item,
                flags: vec![note],
            });
        }
    }
    Ok(built)
}

/// Fetches and unpacks one source (into its parent's folder when it is nested).
fn unpack(source: &Source, work: &Path, download: &Path) -> Result<PathBuf> {
    let archive = fetch::fetch(source, download)?;
    let dir = source.src_dir(work);
    fetch::unpack_verified(&archive, source.sha256, &dir)?;
    Ok(dir)
}

fn compile(
    source: &Source,
    kind: Kind,
    args: &[String],
    src: &Path,
    build: &Path,
    jobs: usize,
    env: &[(String, String)],
) -> Result<()> {
    match kind {
        Kind::Autotools => {
            if source.name == "freetype" {
                // Its configure copies docs/markdown for an out-of-tree build and prints an
                // error when the folder is missing from the release archive.
                std::fs::create_dir_all(src.join("docs/markdown"))?;
            }
            run(&src.join("configure"), args, build, env)?;
            run(Path::new("make"), &[format!("-j{jobs}")], build, env)?;
            run(Path::new("make"), &["install".to_string()], build, env)
        }
        Kind::Meson => {
            let mut setup = vec!["setup".to_string(), ".".to_string()];
            setup.push(src.display().to_string());
            setup.extend(args.iter().cloned());
            run(Path::new("meson"), &setup, build, env)?;
            run(Path::new("meson"), &["compile".to_string()], build, env)?;
            run(Path::new("meson"), &["install".to_string()], build, env)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plan_follows_the_source_table_and_names_mpv_last() {
        let plan = plan(Target::Ubuntu, "/p");
        let names: Vec<&str> = plan.iter().map(|(s, _, _)| s.name).collect();
        let table: Vec<&str> = SOURCES
            .iter()
            .filter(|s| s.nested.is_none())
            .map(|s| s.name)
            .collect();
        assert_eq!(names, table);
        assert!(!names.contains(&"fast_float") && !names.contains(&"vulkan-headers"));
        assert_eq!(plan.last().unwrap().1, Kind::Meson);
        assert!(plan.iter().all(|(_, _, args)| !args.is_empty()));
    }

    // The libmpv build runs on macOS and Ubuntu only; the paths in this test are Unix paths.
    #[cfg(unix)]
    #[test]
    fn the_environment_holds_only_allowed_variables() {
        let inherited = |name: &str| {
            Some(match name {
                "PATH" => "/bin".to_string(),
                _ => format!("leaked-{name}"),
            })
        };
        let env = clean_env(Target::Macos, Path::new("/o"), &inherited);
        let names: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
        for banned in ["RUSTFLAGS", "CFLAGS", "LDFLAGS", "LIBRARY_PATH", "CPATH"] {
            assert!(!names.contains(&banned), "{banned} leaked");
        }
        let get = |name: &str| env.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str());
        assert_eq!(get("PATH"), Some("/bin"));
        assert_eq!(get("PKG_CONFIG_LIBDIR"), Some("/o/lib/pkgconfig"));
        assert_eq!(get("MACOSX_DEPLOYMENT_TARGET"), Some("13.0"));
        let linux = clean_env(Target::Ubuntu, Path::new("/o"), &inherited);
        assert!(linux
            .iter()
            .any(|(k, v)| k == "LD_LIBRARY_PATH" && v == "/o/lib"));
        assert!(!linux.iter().any(|(k, _)| k == "PKG_CONFIG_LIBDIR"));
    }

    #[test]
    fn required_tools_depend_on_the_target_and_the_cpu() {
        let mac_arm = required_tools(Target::Macos, "aarch64");
        assert!(mac_arm.contains(&"meson") && mac_arm.contains(&"install_name_tool"));
        assert!(!mac_arm.contains(&"nasm"));
        assert!(required_tools(Target::Macos, "x86_64").contains(&"nasm"));
        let linux = required_tools(Target::Ubuntu, "x86_64");
        assert!(linux.contains(&"patchelf") && linux.contains(&"readelf"));
        assert!(!linux.contains(&"otool"));
    }

    #[test]
    fn the_recipe_hash_changes_with_the_source_the_flags_the_target_and_the_nested_sources() {
        let source = crate::libmpv::sources::source("libplacebo");
        let nested = [crate::libmpv::sources::source("fast_float")];
        let flags = vec!["-Da=b".to_string()];
        let base = recipe_hash(Target::Ubuntu, source, Kind::Meson, &nested, &flags);
        assert_eq!(base.len(), 64);
        assert_eq!(
            base,
            recipe_hash(Target::Ubuntu, source, Kind::Meson, &nested, &flags)
        );
        let other_flags = vec!["-Da=c".to_string()];
        assert_ne!(
            base,
            recipe_hash(Target::Ubuntu, source, Kind::Meson, &nested, &other_flags)
        );
        assert_ne!(
            base,
            recipe_hash(Target::Macos, source, Kind::Meson, &nested, &flags)
        );
        assert_ne!(
            base,
            recipe_hash(Target::Ubuntu, source, Kind::Meson, &[], &flags)
        );
        let ffmpeg = crate::libmpv::sources::source("ffmpeg");
        assert_ne!(
            base,
            recipe_hash(Target::Ubuntu, ffmpeg, Kind::Meson, &nested, &flags)
        );
    }

    #[test]
    fn markers_live_in_a_hidden_folder_of_the_output() {
        assert_eq!(
            marker_path(Path::new("/o"), "ffmpeg"),
            Path::new("/o/.markers/ffmpeg")
        );
    }
}
