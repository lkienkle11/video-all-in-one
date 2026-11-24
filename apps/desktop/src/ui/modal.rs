//! The application's common Dialog policy and actions.

use gpui_kit::base::TestSupportExt;
use gpui_kit::component::{button::Button, dialog::DialogFooter, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::{div, App, IntoElement, SharedString, Window};
use std::rc::Rc;

type Callback = Rc<dyn Fn(&mut Window, &mut App) -> bool>;

#[derive(Clone)]
pub struct ModalAction {
    id: &'static str,
    label: SharedString,
    callback: Callback,
}

impl ModalAction {
    pub fn new(
        id: &'static str,
        label: impl Into<SharedString>,
        callback: impl Fn(&mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        Self {
            id,
            label: label.into(),
            callback: Rc::new(callback),
        }
    }
}

/// Opens a dismissible modal. Body values are cloned for each Dialog render.
pub fn open_modal(
    window: &mut Window,
    cx: &mut App,
    title: SharedString,
    body: impl IntoElement + Clone + 'static,
    actions: Vec<ModalAction>,
) {
    open_with_policy(
        window,
        cx,
        title,
        body,
        actions,
        Some(ModalAction::new("dismiss", "", |_, _| true)),
    );
}

pub(crate) fn open_with_policy(
    window: &mut Window,
    cx: &mut App,
    title: SharedString,
    body: impl IntoElement + Clone + 'static,
    actions: Vec<ModalAction>,
    dismissal: Option<ModalAction>,
) {
    window.open_dialog(cx, move |dialog, _, _| {
        let dismissal = dismissal.clone();
        let dismissible = dismissal.is_some();
        dialog
            .overlay(true)
            .overlay_closable(dismissible)
            .keyboard(dismissible)
            .close_button(false)
            .on_cancel(move |_, window, cx| {
                dismissal
                    .as_ref()
                    .is_some_and(|action| (action.callback)(window, cx))
            })
            .title(
                div()
                    .id("modal-title")
                    .test_support()
                    .aria_label(title.clone())
                    .child(title.clone()),
            )
            .child(body.clone())
            .footer(DialogFooter::new().children(actions.iter().map(|action| {
                let callback = action.callback.clone();
                Button::new(action.id)
                    .label(action.label.clone())
                    .on_click(move |_, window, cx| {
                        if callback(window, cx) {
                            window.close_dialog(cx);
                        }
                    })
            })))
    });
}
