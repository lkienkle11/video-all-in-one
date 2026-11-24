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
