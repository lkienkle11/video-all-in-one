//! Turning the installed prefix into the package folder: unneeded files removed, every library
//! made relocatable, the licences and the manifest written, and the result checked (LGPL scan,
//! the libmpv API, no path that points back at the build machine).

use super::inspect;
use super::manifest::{self, Built};
use super::steps::run;
use super::{Target, RECIPE_VERSION};
use crate::package::elf;
use crate::package::license::{check_bundle_license, contains};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Finishes the folder `out` after every component is installed into it.
pub(super) fn finish(
    target: Target,
    out: &Path,
    work: &Path,
    built: &[Built],
    env: &[(String, String)],
) -> Result<()> {
    remove_unneeded(out)?;
    let libraries = libraries(out)?;
    for library in &libraries {
        match target {
            Target::Ubuntu => run(
                Path::new("patchelf"),
                &["--set-rpath".into(), "$ORIGIN".into(), path_arg(library)],
                out,
                env,
            )?,
            Target::Macos => relocate_macho(library, out, env)?,
        }
    }
    relocate_pkgconfig(out)?;
    write_licenses(out, work, built)?;
    fs::write(
        out.join("MANIFEST.txt"),
        manifest::manifest(RECIPE_VERSION, target, built),
    )?;
    verify(target, out, work, env)
}

/// Real library files (not symlinks) in `out/lib`.
fn libraries(out: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let lib = out.join("lib");
    for entry in fs::read_dir(&lib).with_context(|| format!("reading {}", lib.display()))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_library = name.ends_with(".dylib") || name.contains(".so");
        if is_library && entry.file_type()?.is_file() {
            found.push(entry.path());
        }
    }
    found.sort();
    Ok(found)
}

fn path_arg(path: &Path) -> String {
    path.display().to_string()
}

/// Programs, manual pages and static libraries are not part of the package.
fn remove_unneeded(out: &Path) -> Result<()> {
    for folder in ["bin", "share", "lib/cmake"] {
        let path = out.join(folder);
        if path.exists() {
            fs::remove_dir_all(&path).with_context(|| format!("removing {}", path.display()))?;
        }
    }
    for entry in fs::read_dir(out.join("lib"))? {
        let path = entry?.path();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        if name.is_some_and(|n| n.ends_with(".la") || n.ends_with(".a")) {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// macOS: the library's own name, and the names of the libraries next to it, become
/// `@rpath/<file>`; its only run path becomes `@loader_path`. The ad-hoc signature is renewed
/// because `install_name_tool` breaks it and arm64 refuses unsigned code.
fn relocate_macho(library: &Path, out: &Path, env: &[(String, String)]) -> Result<()> {
    let name = library
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let mut args: Vec<String> = vec!["-id".into(), format!("@rpath/{name}")];
    let listing = tool_output("otool", &["-L", &path_arg(library)], env)?;
    for dep in inspect::macho_absolute_references(&listing, true) {
        if dep.starts_with(&out.display().to_string()) {
            if let Some(new) = inspect::rpath_reference(&dep) {
                args.extend(["-change".into(), dep, new]);
            }
        }
    }
    let existing = inspect::macho_rpaths(&tool_output("otool", &["-l", &path_arg(library)], env)?);
    let (delete, add) = rpath_edits(&existing);
    for rpath in delete {
        args.extend(["-delete_rpath".into(), rpath]);
    }
    if add {
        args.extend(["-add_rpath".into(), "@loader_path".into()]);
    }
    args.push(path_arg(library));
    run(Path::new("install_name_tool"), &args, out, env)?;
    run(
        Path::new("codesign"),
        &[
            "--force".into(),
            "--sign".into(),
            "-".into(),
            path_arg(library),
        ],
        out,
        env,
    )
}

/// Which run paths to delete and whether to add `@loader_path`, so that running the relocation
/// again on an already relocated library (a rebuild from a cached folder) changes nothing:
/// `install_name_tool` refuses to delete and add the same run path in one call.
fn rpath_edits(existing: &[String]) -> (Vec<String>, bool) {
    let delete = existing
        .iter()
        .filter(|rpath| rpath.as_str() != "@loader_path")
        .cloned()
        .collect();
    (
        delete,
        !existing.iter().any(|rpath| rpath == "@loader_path"),
    )
}

fn tool_output(program: &str, args: &[&str], env: &[(String, String)]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .output()
        .with_context(|| format!("starting {program}"))?;
    if !output.status.success() {
        bail!("{program} {} failed with {}", args.join(" "), output.status);
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The `.pc` files must not name the build folder.
fn relocate_pkgconfig(out: &Path) -> Result<()> {
    let folder = out.join("lib/pkgconfig");
    if !folder.is_dir() {
        return Ok(());
    }
    let prefix = out.display().to_string();
    for entry in fs::read_dir(&folder)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "pc") {
            let text = fs::read_to_string(&path)?;
            fs::write(&path, manifest::relocate_pkgconfig(&text, &prefix))?;
        }
    }
    Ok(())
}

/// The licence texts of every component, and where their sources can be had.
fn write_licenses(out: &Path, work: &Path, built: &[Built]) -> Result<()> {
    let folder = out.join("LICENSES");
    fs::create_dir_all(&folder)?;
    for item in built {
        let src = item.source.src_dir(work);
        for file in item.source.licenses {
            let from = src.join(file);
            let base = Path::new(file)
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let to = folder.join(format!("{}-{base}", item.source.name));
            fs::copy(&from, &to).with_context(|| format!("copying licence {}", from.display()))?;
        }
    }
    fs::write(folder.join("SOURCES.txt"), manifest::sources_notice(built))?;
    Ok(())
}

/// The checks that decide whether the folder may be published.
fn verify(target: Target, out: &Path, work: &Path, env: &[(String, String)]) -> Result<()> {
    let libraries = libraries(out)?;
    // Without the override flag a GPL or non-free library is an error, so this returns no warnings.
    check_bundle_license(&libraries, false)?;
    let mpv = libraries
        .iter()
        .find(|l| {
            l.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("libmpv"))
        })
        .context("no libmpv library was built")?;
    if !contains(&fs::read(mpv)?, b"mpv_render_context_create") {
        bail!(
            "{} has no mpv_render_context_create: the render API is missing",
            mpv.display()
        );
    }
    for library in &libraries {
        match target {
            Target::Ubuntu => {
                let text = tool_output("readelf", &["-d", &path_arg(library)], env)?;
                if elf::run_paths(&text) != ["$ORIGIN"] {
                    bail!("{} must have the run path $ORIGIN only", library.display());
                }
            }
            Target::Macos => {
                let text = tool_output("otool", &["-L", &path_arg(library)], env)?;
                let id = inspect::macho_install_name(&text).unwrap_or_default();
                if !id.starts_with("@rpath/") {
                    bail!(
                        "{} has the install name {id}, not @rpath/...",
                        library.display()
                    );
                }
                let bad = inspect::macho_absolute_references(&text, true);
                if !bad.is_empty() {
                    bail!("{} still points at {}", library.display(), bad.join(", "));
                }
            }
        }
    }
    probe_api_version(out, work, env)
}

/// Compiles a tiny program against the new libmpv and checks that it reports client API 2.
fn probe_api_version(out: &Path, work: &Path, env: &[(String, String)]) -> Result<()> {
    let source = work.join("probe.c");
    let exe = work.join("probe");
    fs::write(&source, inspect::PROBE_SOURCE)?;
    let lib = out.join("lib");
    run(
        Path::new("cc"),
        &[
            path_arg(&source),
            "-I".into(),
            path_arg(&out.join("include")),
            "-L".into(),
            path_arg(&lib),
            "-lmpv".into(),
            format!("-Wl,-rpath,{}", lib.display()),
            "-o".into(),
            path_arg(&exe),
        ],
        work,
        env,
    )?;
    let printed = tool_output(&path_arg(&exe), &[], env)?;
    match inspect::api_major(&printed) {
        Some(2) => {
            println!("libmpv client API version {}", printed.trim());
            Ok(())
        }
        other => bail!("libmpv reports client API {other:?} ({printed:?}), the app needs 2"),
    }
}

#[cfg(test)]
mod rpath_tests {
    use super::rpath_edits;

    #[test]
    fn relocating_twice_changes_nothing_the_second_time() {
        assert_eq!(rpath_edits(&[]), (vec![], true));
        assert_eq!(
            rpath_edits(&["/opt/x/lib".to_string()]),
            (vec!["/opt/x/lib".to_string()], true)
        );
        assert_eq!(rpath_edits(&["@loader_path".to_string()]), (vec![], false));
        assert_eq!(
            rpath_edits(&["@loader_path".to_string(), "/opt/y".to_string()]),
            (vec!["/opt/y".to_string()], false)
        );
    }
}
