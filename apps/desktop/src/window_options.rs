use gpui_kit::{px, size, SharedString, TitlebarOptions, WindowOptions};
use vaio_platform::channel::Channel;

pub const MIN_WINDOW_WIDTH_PX: f32 = 960.0;
pub const MIN_WINDOW_HEIGHT_PX: f32 = 600.0;

pub fn main_window_options(channel: Channel, title: SharedString) -> WindowOptions {
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(title),
            ..Default::default()
        }),
        window_min_size: Some(size(px(MIN_WINDOW_WIDTH_PX), px(MIN_WINDOW_HEIGHT_PX))),
        app_id: Some(channel.identifier().to_owned()),
        ..Default::default()
    }
}
