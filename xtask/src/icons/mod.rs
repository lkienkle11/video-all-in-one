//! `cargo xtask icons`: renders the two app icon sources to PNG and packs them into the
//! `icon.ico`, `icon.icns` and `icon.png` files that the packagers and the window use.
//! `--check` regenerates everything in memory and fails when a committed file differs.

mod icns;
mod ico;
mod render;
#[cfg(test)]
mod tests;

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// The PNG sizes rendered from each SVG. 48 is only used by the `.ico`.
pub(crate) const SIZES: [u32; 8] = [16, 32, 48, 64, 128, 256, 512, 1024];
/// The sizes inside `icon.ico`.
pub(crate) const ICO_SIZES: [u32; 6] = [16, 32, 48, 64, 128, 256];
/// The size of `icon.png`.
const PNG_SIZE: u32 = 512;

/// Which build channel an icon belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Variant {
    Production,
    Development,
}

impl Variant {
    const ALL: [Variant; 2] = [Variant::Production, Variant::Development];

    /// The folder under `assets/icons/generated`.
    pub(crate) fn folder(self) -> &'static str {
        match self {
            Variant::Production => "production",
            Variant::Development => "development",
        }
    }

    /// The source file under `assets/icons`.
    fn source(self) -> &'static str {
        match self {
            Variant::Production => "app-icon.svg",
            Variant::Development => "app-icon-dev.svg",
        }
    }
}

/// The generated file `file` (`icon.ico`, `icon.icns` or `icon.png`) of `variant`.
pub(crate) fn generated_path(root: &Path, variant: Variant, file: &str) -> PathBuf {
    root.join("assets/icons/generated")
        .join(variant.folder())
        .join(file)
}

/// Every generated file as (path relative to `assets/icons/generated`, bytes).
pub(crate) fn generate(icons_dir: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let mut files = Vec::new();
    for variant in Variant::ALL {
        let path = icons_dir.join(variant.source());
        let svg = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let mut pngs = Vec::new();
        for size in SIZES {
            let png = render::render_png(&svg, size)
                .with_context(|| format!("rendering {} at {size}", path.display()))?;
            pngs.push((size, png));
        }
        let pick = |wanted: &[u32]| -> Vec<(u32, Vec<u8>)> {
            pngs.iter()
                .filter(|(size, _)| wanted.contains(size))
                .cloned()
                .collect()
        };
        let folder = PathBuf::from(variant.folder());
        files.push((
            folder.join("icon.ico"),
            ico::ico_container(&pick(&ICO_SIZES))?,
        ));
        files.push((folder.join("icon.icns"), icns::icns_container(&pngs)?));
        let png = pick(&[PNG_SIZE]).remove(0).1;
        files.push((folder.join("icon.png"), png));
    }
    Ok(files)
}

/// Writes the generated files under `root`, or with `check` only compares them with the
/// committed ones and fails when one is missing or different.
pub(crate) fn run(root: &Path, check: bool) -> Result<()> {
    let out = root.join("assets/icons/generated");
    let files = generate(&root.join("assets/icons"))?;
    if check {
        let stale: Vec<String> = files
            .iter()
            .filter(|(path, bytes)| fs::read(out.join(path)).ok().as_deref() != Some(bytes))
            .map(|(path, _)| path.display().to_string())
            .collect();
        if !stale.is_empty() {
            bail!(
                "generated icons are out of date: {}; run `cargo xtask icons`",
                stale.join(", ")
            );
        }
        return Ok(());
    }
    for (path, bytes) in &files {
        let target = out.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::write(&target, bytes).with_context(|| format!("writing {}", target.display()))?;
        println!("wrote {} ({} bytes)", target.display(), bytes.len());
    }
    Ok(())
}
