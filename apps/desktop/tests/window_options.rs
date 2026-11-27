use gpui_kit::{px, size};
use vaio_desktop::window_options::main_window_options;
use vaio_platform::channel::Channel;

#[test]
fn options_set_a_minimum_size() {
    let options = main_window_options(Channel::Development, "Title".into());
    assert_eq!(options.window_min_size, Some(size(px(960.0), px(600.0))));
}

#[test]
fn options_use_the_channel_identifier_as_app_id() {
    let dev = main_window_options(Channel::Development, "Title".into());
    let production = main_window_options(Channel::Production, "Title".into());
    assert_eq!(dev.app_id.as_deref(), Some("app.videoallinone.dev"));
    assert_eq!(production.app_id.as_deref(), Some("app.videoallinone"));
}

#[test]
fn title_is_passed_through() {
    let options = main_window_options(Channel::Development, "Custom title".into());
    assert_eq!(
        options.titlebar.unwrap().title.unwrap().as_ref(),
        "Custom title"
    );
}
