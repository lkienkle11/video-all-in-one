use gpui_kit::{AppContext, TitlebarOptions, WindowOptions};
use vaio_core::i18n::{tr, Language, Text};
use vaio_desktop::app::AppView;
use vaio_platform::channel::Channel;

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            let channel = Channel::current();
            let store = AppView::settings_store(channel);
            let language = store.read().language.unwrap_or(Language::English);

            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(tr(language, Text::AppTitle).into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| AppView::with_store(store, channel, window, cx))
            }) {
                eprintln!("Failed to open application window: {error:#}");
                cx.quit();
            }
        });
}
