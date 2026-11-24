use gpui_kit::component::button::Button;
use gpui_kit::prelude::*;
use gpui_kit::test::{TestQueryExt, TestSupportExt, TestWindowExt};
use gpui_kit::{
    div, point, px, size, AppContext, Bounds, Context, IntoElement, Render, ScrollDelta,
    ScrollHandle, TestAppContext, Window, WindowBounds, WindowOptions,
};
use std::{cell::Cell, rc::Rc};
use vaio_desktop::ui::modal::{open_modal, ModalAction};

struct Background {
    scroll: ScrollHandle,
    clicks: Rc<Cell<usize>>,
}
impl Render for Background {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let clicks = self.clicks.clone();
        div()
            .id("background")
            .test_support()
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(
                Button::new("background-button")
                    .label("Background")
                    .on_click(move |_, _, _| clicks.set(clicks.get() + 1)),
            )
            .child(div().h(px(2000.)))
    }
}
fn open(
    cx: &mut TestAppContext,
) -> (
    gpui_kit::AnyWindowHandle,
    gpui_kit::Entity<Background>,
    Rc<Cell<usize>>,
) {
    cx.update(gpui_kit::init);
    let clicks = Rc::new(Cell::new(0));
    let count = clicks.clone();
    let (window, view) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(800.), px(600.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, cx| {
                cx.new(|_| Background {
                    scroll: ScrollHandle::new(),
                    clicks: count,
                })
            },
        )
        .unwrap()
    });
    (window, view, clicks)
}
fn show(window: &mut Window, cx: &mut gpui_kit::App) {
    open_modal(
        window,
        cx,
        "Test modal".into(),
        "Body",
        vec![
            ModalAction::new("first", "First", |_, _| false),
            ModalAction::new("second", "Second", |_, _| true),
        ],
    );
}

#[gpui_kit::test]
fn modal_blocks_background_scroll_and_clicks(cx: &mut TestAppContext) {
    let (handle, view, clicks) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("background-button", cx);
        assert_eq!(clicks.get(), 1);
        window.scroll(
            "background",
            ScrollDelta::Pixels(point(px(0.), px(-60.))),
            cx,
        );
        assert!(view.read(cx).scroll.offset().y < px(0.));
        view.read(cx).scroll.set_offset(point(px(0.), px(0.)));
        show(window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("background-button", cx);
        window.scroll(
            "background",
            ScrollDelta::Pixels(point(px(0.), px(-60.))),
            cx,
        );
        assert_eq!(clicks.get(), 1);
        assert_eq!(view.read(cx).scroll.offset().y, px(0.));
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("modal-title").is_none());
        window.click("background-button", cx);
        assert_eq!(clicks.get(), 2);
        window.scroll(
            "background",
            ScrollDelta::Pixels(point(px(0.), px(-60.))),
            cx,
        );
        assert!(view.read(cx).scroll.offset().y < px(0.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn modal_keeps_focus_inside(cx: &mut TestAppContext) {
    let (handle, _, _) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("tab", cx);
        assert_eq!(window.find("background-button").focused(), Some(true));
        show(window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        for key in ["tab", "tab", "tab", "shift-tab", "shift-tab", "shift-tab"] {
            window.press(key, cx);
            let focused: Vec<_> = window
                .elements()
                .into_iter()
                .filter(|e| e.focused() == Some(true))
                .collect();
            assert!(!focused.is_empty());
            assert_ne!(window.find("background-button").focused(), Some(true));
            assert!(
                window.find("first").focused() == Some(true)
                    || window.find("second").focused() == Some(true)
            );
        }
        window.click("first", cx);
        assert!(window.find("modal-title").visible());
        window.click("second", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("modal-title").is_none());
        assert_eq!(window.find("background-button").focused(), Some(true));
    })
    .unwrap();
}
