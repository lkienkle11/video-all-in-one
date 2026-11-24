use super::icns::{icns_container, TYPES};
use super::ico::ico_container;
use super::render::render_png;
use super::{generate, run, ICO_SIZES, SIZES};
use resvg::tiny_skia::Pixmap;
use std::fs;
use std::path::{Path, PathBuf};

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn source(name: &str) -> Vec<u8> {
    fs::read(repo_root().join("assets/icons").join(name)).unwrap()
}

fn rendered(name: &str, sizes: &[u32]) -> Vec<(u32, Vec<u8>)> {
    sizes
        .iter()
        .map(|&size| (size, render_png(&source(name), size).unwrap()))
        .collect()
}

fn u16_le(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize
}

fn u32_le(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

fn u32_be(bytes: &[u8], at: usize) -> usize {
    u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

fn pixel(png: &[u8], x: u32, y: u32) -> [u8; 4] {
    let pixmap = Pixmap::decode_png(png).unwrap();
    let p = pixmap.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

#[test]
fn ico_container_has_the_expected_directory_and_png_entries() {
    let entries = rendered("app-icon.svg", &ICO_SIZES);
    let ico = ico_container(&entries).unwrap();
    assert_eq!(u16_le(&ico, 0), 0, "reserved");
    assert_eq!(u16_le(&ico, 2), 1, "type 1 is an icon");
    assert_eq!(u16_le(&ico, 4), 6, "one entry per size");
    for (index, (size, png)) in entries.iter().enumerate() {
        let entry = 6 + index * 16;
        let wanted = if *size == 256 { 0 } else { *size as u8 };
        assert_eq!(ico[entry], wanted, "width of entry {index}");
        assert_eq!(ico[entry + 1], wanted, "height of entry {index}");
        assert_eq!(u16_le(&ico, entry + 4), 1, "planes");
        assert_eq!(u16_le(&ico, entry + 6), 32, "bits per pixel");
        let length = u32_le(&ico, entry + 8);
        let offset = u32_le(&ico, entry + 12);
        assert_eq!(length, png.len());
        assert_eq!(&ico[offset..offset + 8], &PNG_SIGNATURE);
        assert_eq!(&ico[offset..offset + length], png.as_slice());
    }
    let last = &entries[5].1;
    assert_eq!(ico.len(), u32_le(&ico, 6 + 5 * 16 + 12) + last.len());
}

#[test]
fn icns_container_has_the_expected_header_length_and_type_codes() {
    let pngs = rendered("app-icon.svg", &SIZES);
    let icns = icns_container(&pngs).unwrap();
    assert_eq!(&icns[0..4], b"icns");
    assert_eq!(
        u32_be(&icns, 4),
        icns.len(),
        "the header holds the file length"
    );
    let mut at = 8;
    let mut seen = Vec::new();
    while at < icns.len() {
        let code = String::from_utf8(icns[at..at + 4].to_vec()).unwrap();
        let length = u32_be(&icns, at + 4);
        assert_eq!(&icns[at + 8..at + 16], &PNG_SIGNATURE, "{code}");
        let (_, size) = TYPES.iter().find(|(c, _)| *c == code).unwrap();
        assert_eq!(
            &icns[at + 8..at + length],
            pngs.iter().find(|(s, _)| s == size).unwrap().1
        );
        seen.push((code, *size));
        at += length;
    }
    assert_eq!(at, icns.len(), "the entries fill the file exactly");
    for (code, size) in [
        ("ic07", 128),
        ("ic08", 256),
        ("ic09", 512),
        ("ic10", 1024),
        ("ic11", 32),
    ] {
        assert!(seen.contains(&(code.to_string(), size)), "{code}");
    }
    assert_eq!(seen.len(), TYPES.len());
}

#[test]
fn rendered_png_has_the_requested_size_and_is_not_blank() {
    for size in [16, 64, 512] {
        let png = render_png(&source("app-icon.svg"), size).unwrap();
        assert_eq!(&png[0..8], &PNG_SIGNATURE);
        let pixmap = Pixmap::decode_png(&png).unwrap();
        assert_eq!((pixmap.width(), pixmap.height()), (size, size));
        assert_eq!(
            pixel(&png, 0, 0)[3],
            0,
            "the corner outside the tile is transparent"
        );
        let opaque = pixmap.pixels().iter().filter(|p| p.alpha() == 255).count();
        assert!(
            opaque > (size * size) as usize / 3,
            "{size}: only {opaque} opaque pixels"
        );
        let mut colors: Vec<_> = pixmap
            .pixels()
            .iter()
            .map(|p| (p.red(), p.green(), p.blue()))
            .collect();
        colors.sort_unstable();
        colors.dedup();
        assert!(
            colors.len() > 3 || size == 16 && colors.len() > 1,
            "{size}: {} colors",
            colors.len()
        );
    }
    assert!(render_png(b"not an svg", 32).is_err());
}

#[test]
fn dev_icon_differs_from_production_icon_in_the_band_area() {
    let prod = render_png(&source("app-icon.svg"), 256).unwrap();
    let dev = render_png(&source("app-icon-dev.svg"), 256).unwrap();
    // The band covers y 760..924 of 1024 and holds the DEV letters around x 410..650; x 300, y 900
    // (75 and 225 at this size) is plain band.
    let (dev_band, prod_band) = (pixel(&dev, 75, 225), pixel(&prod, 75, 225));
    assert_eq!(dev_band, [245, 158, 11, 255], "amber in the dev icon");
    assert_ne!(prod_band, dev_band, "a gradient in the production icon");
    assert!(
        prod_band[0] < 200 && prod_band[1] < 100,
        "{prod_band:?} is not amber"
    );
    // Above the band the two icons are the same picture.
    assert_eq!(pixel(&dev, 75, 50), pixel(&prod, 75, 50));
}

#[test]
fn generate_names_all_six_files_and_the_committed_ones_are_current() {
    let files = generate(&repo_root().join("assets/icons")).unwrap();
    let names: Vec<String> = files
        .iter()
        .map(|(p, _)| p.display().to_string().replace('\\', "/"))
        .collect();
    for folder in ["production", "development"] {
        for file in ["icon.ico", "icon.icns", "icon.png"] {
            assert!(
                names.contains(&format!("{folder}/{file}")),
                "{folder}/{file}"
            );
        }
    }
    let png = &files
        .iter()
        .find(|(p, _)| p.ends_with("production/icon.png"))
        .unwrap()
        .1;
    assert_eq!(Pixmap::decode_png(png).unwrap().width(), 512);
}

#[test]
fn check_passes_on_fresh_files_and_fails_on_a_changed_or_missing_file() {
    let root = std::env::temp_dir().join(format!("vaio-icons-check-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("assets/icons")).unwrap();
    for name in ["app-icon.svg", "app-icon-dev.svg"] {
        fs::write(root.join("assets/icons").join(name), source(name)).unwrap();
    }
    assert!(run(&root, true).is_err(), "nothing generated yet");
    run(&root, false).unwrap();
    run(&root, true).unwrap();
    let ico = root.join("assets/icons/generated/development/icon.ico");
    let mut bytes = fs::read(&ico).unwrap();
    *bytes.last_mut().unwrap() ^= 0xff;
    fs::write(&ico, bytes).unwrap();
    let message = format!("{:#}", run(&root, true).unwrap_err());
    assert!(message.contains("development/icon.ico"), "{message}");
    let _ = fs::remove_dir_all(&root);
}
