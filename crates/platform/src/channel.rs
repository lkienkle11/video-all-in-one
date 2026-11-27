//! Build channel: development builds and production releases differ in name,
//! identifier and whether they check for updates.

/// The build channel this binary belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Development,
    Production,
}

impl Channel {
    /// The channel of the running build, selected by the `production` feature.
    pub const fn current() -> Channel {
        if cfg!(feature = "production") {
            Channel::Production
        } else {
            Channel::Development
        }
    }

    /// The application name shown to the user.
    pub fn app_name(&self) -> &'static str {
        match self {
            Channel::Development => "Video All-in-One (Dev)",
            Channel::Production => "Video All-in-One",
        }
    }

    /// The bundle identifier, which also names the data folder.
    pub fn identifier(&self) -> &'static str {
        match self {
            Channel::Development => "app.videoallinone.dev",
            Channel::Production => "app.videoallinone",
        }
    }

    /// The executable and package name of the installed app.
    pub fn binary_name(&self) -> &'static str {
        match self {
            Channel::Development => "video-all-in-one-dev",
            Channel::Production => "video-all-in-one",
        }
    }

    /// Whether this channel may contact the update endpoint.
    pub fn updates_enabled(&self) -> bool {
        match self {
            Channel::Development => false,
            Channel::Production => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Channel;

    #[test]
    fn development_has_dev_name_and_identifier() {
        assert_eq!(Channel::Development.app_name(), "Video All-in-One (Dev)");
        assert_eq!(Channel::Development.identifier(), "app.videoallinone.dev");
    }

    #[test]
    fn production_identifier_is_app_videoallinone() {
        assert_eq!(Channel::Production.identifier(), "app.videoallinone");
    }

    #[test]
    fn development_has_updates_disabled() {
        assert!(!Channel::Development.updates_enabled());
    }

    #[test]
    fn dev_and_production_data_folders_differ() {
        let base = std::path::Path::new("/data");
        let dev = crate::paths::data_dir(Channel::Development, base);
        let prod = crate::paths::data_dir(Channel::Production, base);
        assert_ne!(dev, prod);
        assert_eq!(dev, base.join("app.videoallinone.dev"));
        assert_eq!(prod, base.join("app.videoallinone"));
    }

    /// `cargo xtask package` names the installers from these strings.
    #[test]
    fn package_names_of_both_channels_are_fixed() {
        assert_eq!(Channel::Production.binary_name(), "video-all-in-one");
        assert_eq!(Channel::Development.binary_name(), "video-all-in-one-dev");
        assert_eq!(Channel::Production.app_name(), "Video All-in-One");
        assert_eq!(Channel::Development.app_name(), "Video All-in-One (Dev)");
    }

    #[cfg(feature = "production")]
    #[test]
    fn production_feature_selects_production_channel() {
        assert_eq!(Channel::current(), Channel::Production);
    }

    #[cfg(not(feature = "production"))]
    #[test]
    fn default_build_selects_development_channel() {
        assert_eq!(Channel::current(), Channel::Development);
    }
}
