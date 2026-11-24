//! Data folder per build channel, so development and production data never mix.

use std::path::{Path, PathBuf};

use crate::channel::Channel;

/// The data folder of `channel` under `base`.
pub fn data_dir(channel: Channel, base: &Path) -> PathBuf {
    base.join(channel.identifier())
}

#[cfg(test)]
mod tests {
    use super::data_dir;
    use crate::channel::Channel;
    use std::path::Path;

    #[test]
    fn data_dirs_differ_between_channels() {
        let base = Path::new("/data");
        let dev = data_dir(Channel::Development, base);
        let prod = data_dir(Channel::Production, base);

        assert_ne!(dev, prod);
        assert!(!dev.starts_with(&prod));
        assert!(!prod.starts_with(&dev));
    }
}
