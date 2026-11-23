use gpui_kit::{div, Context, IntoElement, Render, Window};

pub struct AppView;

impl AppView {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for AppView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[cfg(test)]
mod tests {
    use super::AppView;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{px, size, AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions};

    #[gpui_kit::test]
    fn window_opens_with_app_view(cx: &mut TestAppContext) {
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
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
            .expect("open app window")
        });

        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .expect("render app window without panic");
        cx.update(|cx| assert_eq!(cx.windows().len(), 1));
    }
}
