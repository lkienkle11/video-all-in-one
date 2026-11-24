use gpui_kit::test::{TestEventExt, TestWindowExt};
use gpui_kit::{px, size, AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions};
use std::{cell::Cell, rc::Rc};
use vaio_core::{
    i18n::Language,
    update::{Asset, AvailableUpdate},
};
use vaio_desktop::ui::update_dialog::UpdateDialog;

struct Background;
impl gpui_kit::Render for Background {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        _: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        gpui_kit::div()
    }
}

#[gpui_kit::test]
fn update_dialog_shows_size_and_notes_in_selected_language(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    for (language, notes, title) in [
        (Language::English, "Fixed playback", "Update now?"),
        (
            Language::Vietnamese,
            "Sửa phát video",
            "Cập nhật ngay bây giờ?",
        ),
    ] {
        let (handle, _) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(800.), px(600.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |_, cx| cx.new(|_| Background),
            )
            .unwrap()
        });
        let now = Rc::new(Cell::new(0));
        let later = Rc::new(Cell::new(0));
        let update = AvailableUpdate {
            version: "1.2.3".parse().unwrap(),
            size: 12_500_000,
            notes_en: "Fixed playback".into(),
            notes_vi: "Sửa phát video".into(),
            asset: Asset {
                url: "https://example.invalid/update".into(),
                size: 12_500_000,
                sha256: "0".repeat(64),
                signature: String::new(),
            },
        };
        for dismissal in ["update-later", "update-now", "escape", "overlay"] {
            let before_now = now.get();
            let before_later = later.get();
            let now_count = now.clone();
            let later_count = later.clone();
            cx.update_window(handle, |_, window, cx| {
                UpdateDialog::open(
                    language,
                    update.clone(),
                    move |_, _| now_count.set(now_count.get() + 1),
                    move |_, _| later_count.set(later_count.get() + 1),
                    window,
                    cx,
                );
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                assert_eq!(window.find("modal-title").label(), Some(title));
                assert_eq!(window.find("update-notes").label(), Some(notes));
                assert!(window
                    .find("update-size")
                    .label()
                    .unwrap()
                    .contains("12.5 MB"));
                assert!(window
                    .find("update-version")
                    .label()
                    .unwrap()
                    .contains("1.2.3"));
                match dismissal {
                    "escape" => window.press("escape", cx),
                    "overlay" => {
                        let point = gpui_kit::point(px(8.), px(100.));
                        window.pointer_down(point, gpui_kit::MouseButton::Left, cx);
                        window.pointer_up(point, gpui_kit::MouseButton::Left, cx);
                    }
                    id => window.click(id, cx),
                }
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                assert!(window.try_find("update-notes").is_none());
            })
            .unwrap();
            assert_eq!(
                now.get(),
                before_now + usize::from(dismissal == "update-now")
            );
            assert_eq!(
                later.get(),
                before_later + usize::from(dismissal != "update-now")
            );
        }
        assert_eq!(now.get(), 1);
        assert_eq!(later.get(), 3);
    }
}
