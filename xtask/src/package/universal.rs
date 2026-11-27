//! `cargo xtask universal`: one macOS disk image that runs on Apple silicon and Intel. It joins
//! the two per-CPU disk images (each with its own libmpv and FFmpeg) file by file with `lipo`.
//! The update zips stay per CPU; only the disk image a new customer opens is universal.
use super::{package_file_name, run_tool, stage_name, BuildChannel};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use vaio_platform::channel::Channel;

/// What to do with one file that exists in both apps.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Copy,
    Lipo,
}

/// A Mach-O file starts with one of the thin or fat magic numbers.
fn is_macho(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..4),
        Some([0xcf, 0xfa, 0xed, 0xfe] | [0xfe, 0xed, 0xfa, 0xcf] | [0xca, 0xfe, 0xba, 0xbe])
    )
}

fn classify(path: &str, arm: &[u8], intel: &[u8]) -> Result<Action> {
    if arm == intel {
        return Ok(Action::Copy);
    }
    if is_macho(arm) && is_macho(intel) {
        return Ok(Action::Lipo);
    }
    bail!("{path} differs between the two apps and is not a program: it cannot be joined")
}

/// The seal of the first signing. It differs between the two apps and the joined app is signed
/// again, so it is dropped.
fn is_signature(path: &str) -> bool {
    path.split('/').any(|part| part == "_CodeSignature")
}

const MINIMUM_KEY: &str = "<key>LSMinimumSystemVersion</key>\n\t<string>";

fn version_parts(text: &str) -> Vec<u32> {
    text.split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

/// The two Info.plist files may differ only in the minimum system version; the universal app
/// needs the higher one, because both halves must run there.
fn merge_plist(arm: &str, intel: &str) -> Result<String> {
    if arm == intel {
        return Ok(arm.to_owned());
    }
    let value = |text: &str| -> Option<String> {
        let start = text.find(MINIMUM_KEY)? + MINIMUM_KEY.len();
        Some(text[start..start + text[start..].find('<')?].to_owned())
    };
    let (Some(a), Some(b)) = (value(arm), value(intel)) else {
        bail!("Info.plist has no LSMinimumSystemVersion");
    };
    let higher = if version_parts(&a) >= version_parts(&b) {
        &a
    } else {
        &b
    };
    let (fixed_arm, fixed_intel) = (
        arm.replacen(
            &format!("{MINIMUM_KEY}{a}<"),
            &format!("{MINIMUM_KEY}{higher}<"),
            1,
        ),
        intel.replacen(
            &format!("{MINIMUM_KEY}{b}<"),
            &format!("{MINIMUM_KEY}{higher}<"),
            1,
        ),
    );
    if fixed_arm != fixed_intel {
        bail!("Info.plist differs between the two apps in more than the minimum system version");
    }
    Ok(fixed_arm)
}

/// Copies the app out of a disk image into `to` and returns its path.
fn unpack_app(dmg: &Path, to: &Path) -> Result<PathBuf> {
    let mount = to.join("mount");
    std::fs::create_dir_all(&mount)?;
    run_tool(
        Command::new("hdiutil")
            .args(["attach", "-nobrowse", "-readonly", "-mountpoint"])
            .arg(&mount)
            .arg(dmg),
    )?;
    let copied = (|| -> Result<PathBuf> {
        let app = std::fs::read_dir(&mount)?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .find(|path| path.extension().is_some_and(|ext| ext == "app"))
            .with_context(|| format!("no app in {}", dmg.display()))?;
        let target = to.join(app.file_name().context("app name")?);
        run_tool(Command::new("ditto").arg(&app).arg(&target))?;
        Ok(target)
    })();
    run_tool(
        Command::new("hdiutil")
            .args(["detach", "-force"])
            .arg(&mount),
    )?;
    copied
}

/// Joins `arm` and `intel` into `dest`: every program through `lipo`, everything else copied.
fn join_apps(arm: &Path, intel: &Path, dest: &Path) -> Result<()> {
    run_tool(Command::new("ditto").arg(arm).arg(dest))?;
    let mut stack = vec![PathBuf::new()];
    while let Some(relative) = stack.pop() {
        for entry in std::fs::read_dir(arm.join(&relative))? {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            let kind = entry.file_type()?;
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() {
                join_file(arm, intel, dest, &path)?;
            }
        }
    }
    Ok(())
}

fn join_file(arm: &Path, intel: &Path, dest: &Path, path: &Path) -> Result<()> {
    let name = path.display().to_string();
    if is_signature(&name) {
        return std::fs::remove_file(dest.join(path)).map_err(Into::into);
    }
    let other = intel.join(path);
    if !other.is_file() {
        bail!("{name} is in the Apple silicon app but not in the Intel app");
    }
    let (a, b) = (std::fs::read(arm.join(path))?, std::fs::read(&other)?);
    if name.ends_with("Info.plist") {
        let merged = merge_plist(&String::from_utf8(a)?, &String::from_utf8(b)?)?;
        return std::fs::write(dest.join(path), merged).map_err(Into::into);
    }
    if classify(&name, &a, &b)? == Action::Lipo {
        run_tool(
            Command::new("lipo")
                .arg("-create")
                .arg(arm.join(path))
                .arg(&other)
                .arg("-output")
                .arg(dest.join(path)),
        )?;
    }
    Ok(())
}

/// Reads both disk images, writes `video-all-in-one-<version>[-dev]-macos-universal.dmg`.
pub fn run(
    arm_dmg: &Path,
    intel_dmg: &Path,
    out: &Path,
    version: &str,
    channel: BuildChannel,
) -> Result<()> {
    let channel = Channel::from(channel);
    let work = out.join(stage_name(channel, "macos-universal"));
    if work.exists() {
        std::fs::remove_dir_all(&work)?;
    }
    std::fs::create_dir_all(work.join("arm"))?;
    std::fs::create_dir_all(work.join("intel"))?;
    let arm = unpack_app(arm_dmg, &work.join("arm"))?;
    let intel = unpack_app(intel_dmg, &work.join("intel"))?;
    let app = work.join(arm.file_name().context("app name")?);
    join_apps(&arm, &intel, &app)?;
    run_tool(
        Command::new("codesign")
            .args(["--force", "--deep", "--sign", "-"])
            .arg(&app),
    )?;
    run_tool(
        Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(&app),
    )?;
    let dmg = out.join(package_file_name(
        channel,
        version,
        "macos-universal",
        "dmg",
    ));
    if dmg.exists() {
        std::fs::remove_file(&dmg)?;
    }
    super::macos::make_dmg(channel, &app, &work, &dmg)?;
    println!("wrote {}", dmg.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const THIN_ARM: [u8; 4] = [0xcf, 0xfa, 0xed, 0xfe];
    const FAT: [u8; 4] = [0xca, 0xfe, 0xba, 0xbe];

    fn plist(minimum: &str) -> String {
        format!("<key>LSMinimumSystemVersion</key>\n\t<string>{minimum}</string>\n<key>A</key>")
    }

    #[test]
    fn mach_o_files_are_recognised_by_their_magic_number() {
        assert!(is_macho(&THIN_ARM));
        assert!(is_macho(&[0xfe, 0xed, 0xfa, 0xcf]));
        assert!(is_macho(&FAT));
        assert!(!is_macho(b"#!/bin/sh\n"));
        assert!(!is_macho(b"ab"));
    }

    #[test]
    fn two_different_programs_are_joined_with_lipo() {
        let mut intel = THIN_ARM.to_vec();
        intel.push(7);
        let mut arm = THIN_ARM.to_vec();
        arm.push(9);
        assert_eq!(
            classify("Contents/MacOS/app", &arm, &intel).unwrap(),
            Action::Lipo
        );
    }

    #[test]
    fn identical_plain_files_are_copied_and_different_ones_are_refused() {
        assert_eq!(
            classify("Contents/Resources/a.txt", b"same", b"same").unwrap(),
            Action::Copy
        );
        let error = classify("Contents/Resources/a.txt", b"arm", b"intel").unwrap_err();
        assert!(error.to_string().contains("a.txt"), "{error}");
    }

    #[test]
    fn the_universal_plist_needs_the_higher_minimum_system_version() {
        let merged = merge_plist(&plist("13.0"), &plist("14.2")).unwrap();
        assert_eq!(merged, plist("14.2"));
        let merged = merge_plist(&plist("15.0"), &plist("14.2")).unwrap();
        assert_eq!(merged, plist("15.0"));
        assert_eq!(
            merge_plist(&plist("13.0"), &plist("13.0")).unwrap(),
            plist("13.0")
        );
        // 10.9 is lower than 10.15, although it is larger as text.
        assert_eq!(
            merge_plist(&plist("10.9"), &plist("10.15")).unwrap(),
            plist("10.15")
        );
    }

    #[test]
    fn the_old_signatures_are_dropped_because_the_app_is_signed_again() {
        assert!(is_signature("Contents/_CodeSignature/CodeResources"));
        assert!(!is_signature("Contents/Resources/CodeResources.txt"));
    }

    #[test]
    fn plists_that_differ_in_anything_else_are_refused() {
        let other = plist("13.0").replace("<key>A</key>", "<key>B</key>");
        assert!(merge_plist(&plist("13.0"), &other).is_err());
    }
}
