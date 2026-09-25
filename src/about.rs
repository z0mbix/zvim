use gpui::{prelude::*, *};
use std::sync::Arc;

pub struct AboutView {
    focus: FocusHandle,
    icon: Arc<Image>,
}

impl AboutView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        cx.observe_window_appearance(window, |_, _, cx| cx.notify())
            .detach();
        cx.observe_global_in::<crate::preferences::AppPreferences>(window, |_, _, cx| cx.notify())
            .detach();
        let icon = zvim::icons::resolve(zvim::icons::DEFAULT_ICON_ID);
        Self {
            focus,
            icon: Arc::new(Image::from_bytes(ImageFormat::Png, icon.png.to_vec())),
        }
    }
}

impl Render for AboutView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = crate::preferences::dark(window, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .p_6()
            .bg(rgb(if dark { 0x1e2127 } else { 0xf5f6f9 }))
            .text_color(rgb(if dark { 0xe8ecf3 } else { 0x202632 }))
            .font_family(if cfg!(target_os = "macos") {
                ".AppleSystemUIFont"
            } else {
                "sans-serif"
            })
            .text_size(px(14.))
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &crate::ui::Close, w, _| w.remove_window()))
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, w, cx| {
                if event.keystroke.key == "escape"
                    || (event.keystroke.key == "w" && event.keystroke.modifiers.platform)
                {
                    w.remove_window();
                    cx.stop_propagation();
                }
            }))
            .child(img(self.icon.clone()).size(px(80.)))
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Zvim"),
            )
            .child(format!("Version {}", env!("CARGO_PKG_VERSION")))
            .child(div().text_center().child(env!("CARGO_PKG_DESCRIPTION")))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(if dark { 0xb1b8c6 } else { 0x5d6573 }))
                    .child(format!("Licensed under {}", env!("CARGO_PKG_LICENSE"))),
            )
    }
}

pub fn open(cx: &mut App) {
    for handle in cx.windows() {
        if handle.downcast::<AboutView>().is_some() {
            let _ = handle.update(cx, |_, window, _| window.activate_window());
            cx.activate(true);
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(360.), px(320.)), cx);
    if let Err(error) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            is_resizable: false,
            titlebar: Some(TitlebarOptions {
                title: Some("About Zvim".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| AboutView::new(window, cx)),
    ) {
        eprintln!("Cannot open About Zvim: {error:#}");
    }
    cx.activate(true);
}
