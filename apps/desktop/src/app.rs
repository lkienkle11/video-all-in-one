use crate::ui::{language_modal::LanguageModal, settings::SettingsView};
use gpui_kit::{AppContext, Context, Entity, IntoElement, Render, Window};
use vaio_core::update::UpdateState;
use vaio_platform::{channel::Channel, paths::data_dir, settings_store::SettingsStore};

pub struct AppView {
    settings: Entity<SettingsView>,
}

impl AppView {
    pub fn with_store(
        store: SettingsStore,
        channel: Channel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let settings =
            cx.new(|cx| SettingsView::new(store, channel, UpdateState::Idle, |_, _, _| {}, cx));
        window.set_window_title(vaio_core::i18n::tr(
            settings.read(cx).language(),
            vaio_core::i18n::Text::AppTitle,
        ));
        if settings.read(cx).needs_language_choice() {
            let settings = settings.clone();
            cx.defer_in(window, move |_, window, cx| {
                LanguageModal::open(settings, window, cx)
            });
        }
        Self { settings }
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let channel = Channel::current();
        Self::with_store(Self::settings_store(channel), channel, window, cx)
    }

    pub fn settings_store(channel: Channel) -> SettingsStore {
        data_base()
            .map(|base| SettingsStore::new(data_dir(channel, &base)))
            .unwrap_or_else(SettingsStore::in_memory)
    }
}

impl Render for AppView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.settings.clone()
    }
}

fn data_base() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(std::path::PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(|home| std::path::PathBuf::from(home).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".local/share"))
        });
    base.filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::AppView;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{px, size, AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions};

    #[gpui_kit::test]
    fn window_opens_with_app_view(cx: &mut TestAppContext) {
        struct Directory(std::path::PathBuf);
        impl Drop for Directory {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = Directory(std::env::temp_dir().join(
            format!("vaio-app-{}-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
        ));
        cx.update(gpui_kit::init);
        let (handle, _view) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(640.), px(480.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        AppView::with_store(
                            vaio_platform::settings_store::SettingsStore::new(&dir.0),
                            vaio_platform::channel::Channel::Development,
                            window,
                            cx,
                        )
                    })
                },
            )
            .expect("open app window")
        });

        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .expect("render app window without panic");
        cx.update(|cx| assert_eq!(cx.windows().len(), 1));
    }
}
