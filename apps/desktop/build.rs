//! Embeds the app icon into the Windows executable. Does nothing on macOS and Linux.

fn main() {
    #[cfg(windows)]
    {
        windows::embed_icon();
    }
}

#[cfg(windows)]
mod windows {
    use std::path::PathBuf;

    /// The production icon for a `production` build, the development icon otherwise.
    pub fn embed_icon() {
        let manifest_dir = PathBuf::from(
            std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
        );
        let icons = manifest_dir.join("../../assets/icons/generated");
        for channel in ["production", "development"] {
            println!(
                "cargo:rerun-if-changed={}",
                icons.join(channel).join("icon.ico").display()
            );
        }
        let channel = if std::env::var_os("CARGO_FEATURE_PRODUCTION").is_some() {
            "production"
        } else {
            "development"
        };
        let icon = icons.join(channel).join("icon.ico");
        winresource::WindowsResource::new()
            .set_icon(icon.to_str().expect("the icon path is UTF-8"))
            .compile()
            .expect("embedding the app icon in the Windows executable");
    }
}
