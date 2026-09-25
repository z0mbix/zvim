// Adapted from gpui-libghostty src/adapter.rs at bfa3771f.
// MIT licence: see packaging/GPUI-GHOSTTY-LICENSE.txt.
use gpui::{
    AppContext as _, Bounds, ClipboardItem, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, KeyUpEvent, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement as _, Pixels, Render, ScrollDelta, ScrollWheelEvent, Styled as _, Subscription,
    Task, Window, canvas, div,
};

use gpui_libghostty::__private::{KeyAction, Modifiers, MouseButton, MouseState, NativeSurface};
use gpui_libghostty::TerminalOptions;
/// A GPUI entity backed by Ghostty's native Metal or Wayland/OpenGL surface.
pub struct Terminal {
    surface: NativeSurface,
    applied_theme: Option<std::sync::Arc<str>>,
    modal_snapshot: Option<std::sync::Arc<gpui::RenderImage>>,
    focus: FocusHandle,
    bounds: Bounds<Pixels>,
    tick_task: Option<Task<()>>,
    visible: bool,
    exit_reported: bool,
    window_focused: bool,
    _subscriptions: Vec<Subscription>,
}

pub struct TerminalExited;
impl gpui::EventEmitter<TerminalExited> for Terminal {}

pub struct TerminalFocused;
impl gpui::EventEmitter<TerminalFocused> for Terminal {}

impl Terminal {
    /// Spawns the configured command and attaches its native surface to `window`.
    pub fn spawn<T: 'static>(
        options: TerminalOptions,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Result<Entity<Self>, String> {
        let focus_on_spawn = options.focus_on_spawn;
        // SAFETY: GPUI owns the parent window; this entity services and drops
        // its native child on the window's UI thread, as in the native adapter.
        let surface = unsafe {
            gpui_libghostty::__private::spawn_surface(
                options,
                window,
                f64::from(window.scale_factor()),
            )?
        };
        let focus = cx.focus_handle();
        if focus_on_spawn {
            focus.focus(window);
        }
        Ok(cx.new(|cx| {
            let subscriptions = vec![
                cx.on_focus(&focus, window, |terminal: &mut Self, window, cx| {
                    terminal.sync_focus(window);
                    cx.emit(TerminalFocused);
                }),
                cx.on_blur(&focus, window, |terminal: &mut Self, window, _| {
                    terminal.sync_focus(window)
                }),
                cx.observe_window_activation(window, |terminal: &mut Self, window, _| {
                    terminal.sync_focus(window)
                }),
            ];
            let mut terminal = Self {
                surface,
                applied_theme: None,
                modal_snapshot: None,
                focus,
                bounds: Bounds::default(),
                tick_task: None,
                visible: true,
                exit_reported: false,
                window_focused: false,
                _subscriptions: subscriptions,
            };
            terminal.sync_focus(window);
            terminal
        }))
    }

    pub fn set_theme(&mut self, config: Option<std::sync::Arc<str>>) -> Result<(), String> {
        if self.applied_theme != config {
            self.surface.set_color_config(config.as_deref())?;
            self.applied_theme = config;
        }
        Ok(())
    }

    pub fn needs_confirm_quit(&self) -> bool {
        self.is_alive() && self.surface.needs_confirm_quit()
    }

    pub fn is_alive(&self) -> bool {
        self.surface.is_alive()
    }

    pub fn focus<T>(&mut self, window: &mut Window, _cx: &mut Context<T>) {
        self.set_visible(true);
        self.focus.focus(window);
        self.sync_focus(window);
    }

    /// Shows or hides the native child without changing GPUI keyboard focus.
    /// Hidden surfaces remain hidden across layout, resize, and scale changes.
    pub fn prepare_close_prompt(&mut self, cx: &mut Context<Self>) {
        let snapshot = self
            .surface
            .snapshot_frame()
            .ok()
            .map(|frame| std::sync::Arc::new(gpui::RenderImage::new([frame])));
        self.set_visible(false);
        self.modal_snapshot = snapshot;
        cx.notify();
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.modal_snapshot = None;
        self.visible = visible;
        self.surface
            .set_visible(visible && self.bounds.size.width > gpui::px(0.));
        self.surface.set_focus(self.visible && self.window_focused);
    }

    fn sync_focus(&mut self, window: &Window) {
        self.window_focused = self.focus.is_focused(window) && window.is_window_active();
        self.surface.set_focus(self.visible && self.window_focused);
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }

    pub fn paste(&mut self) {
        let mut modifiers = Modifiers::SUPER;
        if cfg!(target_os = "linux") {
            modifiers = Modifiers::CONTROL;
            modifiers.insert(Modifiers::SHIFT);
        }
        self.surface
            .send_key(KeyAction::Press, "v", None, modifiers);
        self.surface
            .send_key(KeyAction::Release, "v", None, modifiers);
    }

    fn start_ticking(&mut self, cx: &mut Context<Self>) {
        if self.tick_task.is_some() {
            return;
        }
        self.tick(cx);
        let wakeup = self.surface.wakeup();
        let terminal = cx.entity().downgrade();
        self.tick_task = Some(cx.spawn(async move |_, cx| {
            loop {
                wakeup.wait().await;
                let updated = terminal.update(cx, |terminal, cx| {
                    // Ghostty draws its native child during the tick; GPUI has no
                    // terminal pixels to repaint for this wakeup.
                    terminal.tick(cx);
                });
                if updated.is_err() {
                    break;
                }
            }
        }));
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        self.surface.tick();
        self.service_clipboard(cx);
        if !self.exit_reported && !self.is_alive() {
            self.exit_reported = true;
            cx.emit(TerminalExited);
        }
    }

    fn service_clipboard(&mut self, cx: &mut Context<Self>) {
        self.surface.service_clipboard_read(|selection| {
            let item = if selection {
                #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                {
                    cx.read_from_primary()
                }
                #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
                {
                    cx.read_from_clipboard()
                }
            } else {
                cx.read_from_clipboard()
            };
            item.and_then(|item| item.text()).unwrap_or_default()
        });

        while let Some(write) = self.surface.take_clipboard_write() {
            let item = ClipboardItem::new_string(write.text);
            if write.selection {
                #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                cx.write_to_primary(item);
                #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
                cx.write_to_clipboard(item);
            } else {
                cx.write_to_clipboard(item);
            }
        }
    }

    fn update_frame(&mut self, bounds: Bounds<Pixels>, scale_factor: f64) {
        let first_frame = self.bounds.size.width == gpui::px(0.);
        self.bounds = bounds;
        self.surface.set_frame(
            f64::from(f32::from(bounds.origin.x)),
            f64::from(f32::from(bounds.origin.y)),
            f64::from(f32::from(bounds.size.width)),
            f64::from(f32::from(bounds.size.height)),
            scale_factor,
        );
        if first_frame {
            self.surface.set_visible(self.visible);
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent) {
        self.send_key(
            if event.is_held {
                KeyAction::Repeat
            } else {
                KeyAction::Press
            },
            &event.keystroke,
        );
    }

    fn key_up(&mut self, event: &KeyUpEvent) {
        self.send_key(KeyAction::Release, &event.keystroke);
    }

    fn send_key(&mut self, action: KeyAction, keystroke: &gpui::Keystroke) {
        self.surface.send_key(
            action,
            &keystroke.key,
            keystroke.key_char.as_deref(),
            modifiers(keystroke.modifiers),
        );
    }

    fn mouse_position(&mut self, position: gpui::Point<Pixels>, input_modifiers: gpui::Modifiers) {
        let x = f64::from(f32::from(position.x - self.bounds.origin.x));
        let y = f64::from(f32::from(position.y - self.bounds.origin.y));
        self.surface
            .mouse_position(x, y, modifiers(input_modifiers));
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, _cx: &mut Context<Self>) {
        self.focus.focus(window);
        self.sync_focus(window);
        self.mouse_position(event.position, event.modifiers);
        self.surface.mouse_button(
            MouseState::Press,
            mouse_button(event.button),
            modifiers(event.modifiers),
        );
    }

    fn mouse_up(&mut self, event: &MouseUpEvent) {
        self.mouse_position(event.position, event.modifiers);
        self.surface.mouse_button(
            MouseState::Release,
            mouse_button(event.button),
            modifiers(event.modifiers),
        );
    }

    fn scroll(&mut self, event: &ScrollWheelEvent) {
        self.mouse_position(event.position, event.modifiers);
        let (x, y, precision) = match event.delta {
            ScrollDelta::Pixels(delta) => (
                f64::from(f32::from(delta.x)),
                f64::from(f32::from(delta.y)),
                true,
            ),
            ScrollDelta::Lines(delta) => (f64::from(delta.x), f64::from(delta.y), false),
        };
        self.surface.mouse_scroll(x, y, precision);
    }
}

impl Render for Terminal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_focus(window);
        self.start_ticking(cx);
        let terminal = cx.entity().downgrade();
        let mut element = div()
            .relative()
            .key_context("Terminal")
            .track_focus(&self.focus)
            .size_full()
            .min_h_0()
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let scale_factor = f64::from(window.scale_factor());
                        let _ = terminal.update(cx, |terminal, _| {
                            terminal.update_frame(bounds, scale_factor);
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .on_key_down(cx.listener(|terminal, event, _, cx| {
                terminal.key_down(event);
                cx.stop_propagation();
            }))
            .on_key_up(cx.listener(|terminal, event, _, cx| {
                terminal.key_up(event);
                cx.stop_propagation();
            }))
            .on_mouse_move(cx.listener(|terminal, event: &MouseMoveEvent, _, _| {
                terminal.mouse_position(event.position, event.modifiers);
            }));
        if let Some(snapshot) = &self.modal_snapshot {
            element = element.child(gpui::img(snapshot.clone()).absolute().size_full());
        }
        for button in [
            gpui::MouseButton::Left,
            gpui::MouseButton::Middle,
            gpui::MouseButton::Right,
        ] {
            element = element
                .on_mouse_down(
                    button,
                    cx.listener(|terminal, event, window, cx| {
                        terminal.mouse_down(event, window, cx)
                    }),
                )
                .on_mouse_up(
                    button,
                    cx.listener(|terminal, event, _, _| terminal.mouse_up(event)),
                );
        }
        element.on_scroll_wheel(cx.listener(|terminal, event, _, _| terminal.scroll(event)))
    }
}

fn modifiers(value: gpui::Modifiers) -> Modifiers {
    let mut result = Modifiers::empty();
    if value.shift {
        result.insert(Modifiers::SHIFT);
    }
    if value.control {
        result.insert(Modifiers::CONTROL);
    }
    if value.alt {
        result.insert(Modifiers::ALT);
    }
    if value.platform {
        result.insert(Modifiers::SUPER);
    }
    result
}

fn mouse_button(value: gpui::MouseButton) -> MouseButton {
    match value {
        gpui::MouseButton::Left => MouseButton::Left,
        gpui::MouseButton::Right => MouseButton::Right,
        gpui::MouseButton::Middle => MouseButton::Middle,
        _ => MouseButton::Unknown,
    }
}

/// Called before GPUI, Neovim or Ghostty starts any worker threads.
pub fn configure_resources() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    let candidates = [
        dir.join("../Resources/ghostty"),
        dir.join("share/ghostty"),
        dir.join("../ghostty-resources/ghostty"),
    ];
    if let Some(path) = candidates
        .into_iter()
        .find(|path| path.join("themes").is_dir())
    {
        // SAFETY: main calls this before creating any application threads.
        unsafe {
            std::env::set_var("GHOSTTY_RESOURCES_DIR", path);
        }
    }
}
