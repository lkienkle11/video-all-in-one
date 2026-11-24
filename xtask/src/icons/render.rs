//! Renders an SVG to a square PNG with resvg. The icon sources have no text, so no font is needed.

use anyhow::{anyhow, bail, Result};
use resvg::{tiny_skia, usvg};

/// `svg` drawn on a transparent `size` x `size` canvas and encoded as PNG.
pub(super) fn render_png(svg: &[u8], size: u32) -> Result<Vec<u8>> {
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default())
        .map_err(|error| anyhow!("the SVG is not valid: {error}"))?;
    let width = tree.size().width();
    if width <= 0.0 || tree.size().width() != tree.size().height() {
        bail!("an icon source must be a square image");
    }
    let mut pixmap = tiny_skia::Pixmap::new(size, size)
        .ok_or_else(|| anyhow!("cannot make a {size}x{size} canvas"))?;
    let scale = size as f32 / width;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|error| anyhow!("encoding the PNG failed: {error}"))
}
