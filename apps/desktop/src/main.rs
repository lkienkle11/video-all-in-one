mod app;

use app::AppView;
use gpui_kit::{AppContext, TitlebarOptions, WindowOptions};

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);

            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Video All-in-One".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| AppView::new(window, cx))
            }) {
                eprintln!("Failed to open application window: {error:#}");
                cx.quit();
            }
        });
}
