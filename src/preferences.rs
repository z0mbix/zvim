use gpui::{prelude::*, *};
use std::sync::Arc;
use zvim::icons::{APP_ICONS, resolve};
use zvim::settings::{Preferences, Theme};

pub struct AppPreferences(pub Preferences);
impl Global for AppPreferences {}

pub fn bind_keys(cx: &mut App) {
    let p = &cx.global::<AppPreferences>().0;
    let (toggle, maximize) =
        if validate_shortcuts(&p.terminal_toggle_key, &p.terminal_maximize_key).is_ok() {
            (
                p.terminal_toggle_key.clone(),
                p.terminal_maximize_key.clone(),
            )
        } else {
            let defaults = Preferences::default();
            (defaults.terminal_toggle_key, defaults.terminal_maximize_key)
        };
    cx.clear_key_bindings();
    cx.bind_keys([
        KeyBinding::new(&toggle, crate::ui::ToggleTerminal, Some("ZvimWindow")),
        KeyBinding::new(&maximize, crate::ui::MaximizeTerminal, Some("ZvimWindow")),
        KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-,"
            } else {
                "ctrl-shift-,"
            },
            crate::ui::OpenSettings,
            None,
        ),
        KeyBinding::new("cmd-w", crate::ui::Close, Some("ZvimWindow")),
        KeyBinding::new("cmd-q", crate::ui::Quit, None),
        KeyBinding::new("cmd-n", crate::ui::NewWindow, None),
    ]);
}

fn validate_shortcuts(toggle: &str, maximize: &str) -> anyhow::Result<()> {
    let parse = |value: &str| -> anyhow::Result<Keystroke> {
        let key = Keystroke::parse(value).map_err(|e| anyhow::anyhow!("{e}"))?;
        anyhow::ensure!(
            key.modifiers.control || key.modifiers.alt || key.modifiers.platform,
            "Include Control, Option/Alt or Command/Super in the shortcut."
        );
        for reserved in [
            "cmd-w",
            "cmd-q",
            "cmd-n",
            "cmd-o",
            "cmd-v",
            "cmd-,",
            "ctrl-shift-,",
        ] {
            let reserved = Keystroke::parse(reserved).unwrap();
            anyhow::ensure!(
                key.key != reserved.key || key.modifiers != reserved.modifiers,
                "That shortcut is reserved by Zvim."
            );
        }
        Ok(key)
    };
    let a = parse(toggle)?;
    let b = parse(maximize)?;
    anyhow::ensure!(
        a.key != b.key || a.modifiers != b.modifiers,
        "Show/hide and maximise/restore need different shortcuts."
    );
    Ok(())
}

pub fn refresh(cx: &mut App) {
    if let Ok(p) = Preferences::load()
        && p != cx.global::<AppPreferences>().0
    {
        cx.set_global(AppPreferences(p));
    }
}
pub fn dark(window: &Window, cx: &App) -> bool {
    cx.global::<AppPreferences>().0.theme.is_dark(matches!(
        window.appearance(),
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    ))
}

#[derive(Clone, Copy)]
enum Choice {
    Theme(Theme),
    Geometry,
    Sync,
    TerminalTheme,
    Diagnostics,
    CliDirectory,
    CliDefault,
    CliInstall,
    Icon(&'static str),
    RecordShortcut(bool),
    ResetShortcuts,
}

pub struct PreferencesView {
    recording_shortcut: Option<bool>,
    _shortcut_interceptor: Subscription,
    focus: FocusHandle,
    buttons: Vec<FocusHandle>,
    error: Option<String>,
    cli_message: Option<String>,
    cli_installing: bool,
    icon_images: std::collections::HashMap<&'static str, Arc<Image>>,
}
impl PreferencesView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        cx.observe_global_in::<AppPreferences>(window, |_, _, cx| cx.notify())
            .detach();
        cx.observe_window_appearance(window, |_, _, cx| cx.notify())
            .detach();
        cx.observe_window_activation(window, |_, _, cx| refresh(cx))
            .detach();
        let view = cx.entity().downgrade();
        let shortcut_interceptor = cx.intercept_keystrokes(move |_, window, cx| {
            if let Some(view) = view.upgrade() {
                let view = view.read(cx);
                if view.recording_shortcut.is_some() && view.focus.contains_focused(window, cx) {
                    cx.stop_propagation();
                }
            }
        });
        Self {
            _shortcut_interceptor: shortcut_interceptor,
            icon_images: APP_ICONS
                .iter()
                .map(|icon| {
                    (
                        icon.id,
                        Arc::new(Image::from_bytes(ImageFormat::Png, icon.png.to_vec())),
                    )
                })
                .collect(),
            recording_shortcut: None,
            focus,
            cli_message: None,
            cli_installing: false,
            buttons: (0..(13 + APP_ICONS.len()) as isize)
                .map(|i| cx.focus_handle().tab_index(i).tab_stop(true))
                .collect(),
            error: Preferences::load()
                .err()
                .map(|e| format!("Cannot read settings: {e}")),
        }
    }
    fn choose(&mut self, choice: Choice, cx: &mut Context<Self>) {
        match choice {
            Choice::RecordShortcut(maximize) => {
                self.recording_shortcut = Some(maximize);
                self.error = None;
                cx.notify();
                return;
            }
            Choice::CliDirectory => {
                if self.cli_installing {
                    return;
                }
                let result = cx.prompt_for_paths(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Choose bin directory".into()),
                });
                cx.spawn(async move |view, cx| {
                    let result = result.await;
                    let _ = view.update(cx, |view, cx| match result {
                        Ok(Ok(Some(paths))) => {
                            if let Some(path) = paths.into_iter().next() {
                                view.save_cli_directory(path, cx);
                            }
                        }
                        Ok(Ok(None)) => {}
                        _ => {
                            view.cli_message =
                                Some("Could not open the folder chooser. Please try again.".into());
                            cx.notify();
                        }
                    });
                })
                .detach();
                return;
            }
            Choice::CliDefault => {
                if !self.cli_installing {
                    self.save_cli_directory(zvim::cli_install::default_bin_directory(), cx);
                }
                return;
            }
            Choice::CliInstall => {
                if self.cli_installing {
                    return;
                }
                self.cli_installing = true;
                self.cli_message = None;
                let directory = cx.global::<AppPreferences>().0.cli_bin_directory.clone();
                let task = cx
                    .background_executor()
                    .spawn(async move { zvim::cli_install::install_current(&directory) });
                cx.spawn(async move |view, cx| {
                    let result = task.await;
                    let _ = view.update(cx, |view, cx| {
                        view.cli_installing = false;
                        view.cli_message = Some(match result {
                            Ok(path) => format!("Installed {}. You can now use zvim from a shell with this directory on PATH.", path.display()),
                            Err(error) => format!("Could not install: {error:#}"),
                        });
                        cx.notify();
                    });
                }).detach();
                cx.notify();
                return;
            }
            _ => {}
        }
        if matches!(choice, Choice::Diagnostics) {
            let dir = zvim::settings::data_dir();
            match std::fs::create_dir_all(&dir) {
                Ok(()) => cx.reveal_path(&dir),
                Err(e) => self.error = Some(format!("Cannot open diagnostics: {e}")),
            }
            cx.notify();
            return;
        }
        let result = (|| -> anyhow::Result<()> {
            let mut p = Preferences::load()?;
            match choice {
                Choice::ResetShortcuts => {
                    let defaults = Preferences::default();
                    p.terminal_toggle_key = defaults.terminal_toggle_key;
                    p.terminal_maximize_key = defaults.terminal_maximize_key;
                    self.recording_shortcut = None;
                }
                Choice::RecordShortcut(_) => unreachable!(),
                Choice::Theme(theme) => p.theme = theme,
                Choice::Icon(id) => p.app_icon = id.into(),
                Choice::Geometry => p.remember_window_geometry = !p.remember_window_geometry,
                Choice::TerminalTheme => p.terminal_follow_neovim = !p.terminal_follow_neovim,
                Choice::Sync => p.sync_editor_appearance = !p.sync_editor_appearance,
                Choice::Diagnostics
                | Choice::CliDirectory
                | Choice::CliDefault
                | Choice::CliInstall => unreachable!(),
            }
            p.save()?;
            cx.set_global(AppPreferences(p));
            Ok(())
        })();
        self.error = result
            .err()
            .map(|e| format!("Settings were not saved: {e}"));
        cx.notify();
    }
    fn record_shortcut(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(maximize) = self.recording_shortcut else {
            return;
        };
        cx.stop_propagation();
        if event.keystroke.key == "escape" {
            self.recording_shortcut = None;
            cx.notify();
            return;
        }
        if matches!(
            event.keystroke.key.as_str(),
            "shift" | "control" | "alt" | "platform" | "function"
        ) {
            return;
        }
        if event.is_held {
            return;
        }
        let result = (|| -> anyhow::Result<()> {
            let mut p = Preferences::load()?;
            if maximize {
                p.terminal_maximize_key = event.keystroke.unparse();
            } else {
                p.terminal_toggle_key = event.keystroke.unparse();
            }
            validate_shortcuts(&p.terminal_toggle_key, &p.terminal_maximize_key)?;
            p.save()?;
            cx.set_global(AppPreferences(p));
            self.recording_shortcut = None;
            Ok(())
        })();
        self.error = result.err().map(|e| e.to_string());
        cx.notify();
    }

    fn save_cli_directory(&mut self, directory: std::path::PathBuf, cx: &mut Context<Self>) {
        let result = (|| -> anyhow::Result<()> {
            let mut preferences = Preferences::load()?;
            preferences.cli_bin_directory = directory;
            preferences.save()?;
            cx.set_global(AppPreferences(preferences));
            Ok(())
        })();
        self.cli_message = result
            .err()
            .map(|error| format!("Directory was not saved: {error}"));
        cx.notify();
    }

    fn button(
        &self,
        index: usize,
        label: &'static str,
        selected: bool,
        choice: Choice,
        is_dark: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let focus = self.buttons[index].clone();
        div()
            .id(index)
            .track_focus(&focus)
            .tab_index(0)
            .px_4()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(if selected {
                0x6789ee
            } else if is_dark {
                0x414650
            } else {
                0xd0d5df
            }))
            .bg(rgb(if selected {
                if is_dark { 0x33456e } else { 0xe3eaff }
            } else if is_dark {
                0x272b33
            } else {
                0xffffff
            }))
            .cursor_pointer()
            .focus(|s| s.border_color(rgb(0x8ba9ff)).border_2())
            .hover(|s| s.bg(rgb(if is_dark { 0x3c465c } else { 0xe8edfa })))
            .on_click(cx.listener(move |v, _, w, cx| {
                w.focus(&v.buttons[index]);
                v.choose(choice, cx);
            }))
            .on_key_down(cx.listener(move |v, e: &KeyDownEvent, _, cx| {
                if matches!(e.keystroke.key.as_str(), "enter" | "space") {
                    v.choose(choice, cx);
                    cx.stop_propagation();
                }
            }))
            .child(label)
    }
}
impl Render for PreferencesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.global::<AppPreferences>().0.clone();
        let d = dark(window, cx);
        let muted = rgb(if d { 0xb1b8c6 } else { 0x5d6573 });
        div().size_full().id("settings").overflow_y_scroll().p_8().flex().flex_col().gap_6()
            .font_family(if cfg!(target_os="macos") { ".AppleSystemUIFont" } else { "sans-serif" })
            .text_size(px(14.)).bg(rgb(if d {0x1e2127} else {0xf5f6f9})).text_color(rgb(if d {0xe8ecf3} else {0x202632}))
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(|v, e, _, cx| v.record_shortcut(e, cx)))
            .on_action(cx.listener(|_, _: &crate::ui::Close, w, _| w.remove_window()))
            .on_key_down(cx.listener(|_, e: &KeyDownEvent, w, cx| {
                match e.keystroke.key.as_str() {
                    "escape" => w.remove_window(),
                    "tab" => { if e.keystroke.modifiers.shift { w.focus_prev(); } else { w.focus_next(); } },
                    "w" if e.keystroke.modifiers.platform => w.remove_window(),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child("Settings"))
            .child(div().flex().flex_col().gap_3()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Appearance"))
                .child(div().flex().gap_2()
                    .child(self.button(0,"System",p.theme==Theme::System,Choice::Theme(Theme::System),d,cx))
                    .child(self.button(1,"Light",p.theme==Theme::Light,Choice::Theme(Theme::Light),d,cx))
                    .child(self.button(2,"Dark",p.theme==Theme::Dark,Choice::Theme(Theme::Dark),d,cx)))
                .child(div().text_color(muted).child("System follows your desktop appearance. Native titlebars follow the operating system."))
                .child(div().flex().items_center().justify_between().gap_4()
                    .child("Sync editor appearance")
                    .child(self.button(3, if p.sync_editor_appearance {"On"} else {"Off"},p.sync_editor_appearance,Choice::Sync,d,cx)))
                .child(div().text_color(muted).child("Sets Neovim’s light/dark background for compatible colourschemes. NvChad/Base46 palettes stay unchanged. When off, your configuration controls editor colours.")))
            .child(div().border_t_1().border_color(rgb(if d {0x3b4049} else {0xd9dee7})).pt_5().flex().flex_col().gap_3()
                .child(div().flex().items_center().justify_between().gap_4()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Remember window size and position"))
                    .child(self.button(4, if p.remember_window_geometry {"On"} else {"Off"},p.remember_window_geometry,Choice::Geometry,d,cx)))
                .child(div().text_color(muted).child("New windows use the last saved placement. When off, they open centred at the default size. Files and sessions are not restored.")))
            .child(div().flex().flex_col().gap_3()
                .child(div().flex().items_center().justify_between().gap_4()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Terminal theme"))
                    .child(self.button(5, if p.terminal_follow_neovim {"Follow Neovim"} else {"Use Ghostty theme"},p.terminal_follow_neovim,Choice::TerminalTheme,d,cx)))
                .child(div().text_color(muted).child("Updates terminal colours live. Fonts, shell settings and terminal shortcuts use your Ghostty configuration. Pane shortcuts are set below.")))
            .child(div().flex().flex_col().gap_3()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Terminal keybindings"))
                .child(div().flex().items_center().justify_between().gap_4()
                    .child(format!("Show / hide: {}", p.terminal_toggle_key))
                    .child(self.button(10+APP_ICONS.len(), if self.recording_shortcut == Some(false) {"Press shortcut…"} else {"Change"}, false, Choice::RecordShortcut(false), d, cx)))
                .child(div().flex().items_center().justify_between().gap_4()
                    .child(format!("Maximise / restore: {}", p.terminal_maximize_key))
                    .child(self.button(11+APP_ICONS.len(), if self.recording_shortcut == Some(true) {"Press shortcut…"} else {"Change"}, false, Choice::RecordShortcut(true), d, cx)))
                .child(div().flex().child(self.button(12+APP_ICONS.len(), "Reset shortcuts", false, Choice::ResetShortcuts, d, cx)))
                .child(div().text_color(muted).child("Click Change, then press a shortcut. Escape cancels. Changes apply immediately in the editor and terminal. Maximise fills the window; restore returns to your split size.")))
            .child(div().flex().flex_col().gap_3()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Application icon"))
                .child(div().flex().items_center().gap_4()
                    .child(img(self.icon_images[resolve(&p.app_icon).id].clone())
                        .w(px(64.)).h(px(64.)))
                    .children(APP_ICONS.iter().enumerate().map(|(i, icon)|
                        self.button(6+i, icon.label, resolve(&p.app_icon).id == icon.id, Choice::Icon(icon.id), d, cx))))
                .child(div().text_color(muted).child("The Neovim icon is included in this build. More icon choices can be added in future releases.")))
            .child(div().flex().flex_col().gap_3()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Command-line launcher"))
                .child(div().text_color(muted).child("Install the zvim command for this app. No Python or additional tools are needed."))
                .child(div().child(format!("Bin directory: {}", p.cli_bin_directory.display())))
                .child(div().flex().flex_wrap().gap_2()
                    .child(self.button(6+APP_ICONS.len(), "Choose folder…", false, Choice::CliDirectory, d, cx))
                    .child(self.button(7+APP_ICONS.len(), "Use default", false, Choice::CliDefault, d, cx))
                    .child(self.button(8+APP_ICONS.len(), if self.cli_installing {"Installing…"} else {"Install / update zvim"}, false, Choice::CliInstall, d, cx)))
                .child(div().text_color(muted).child("Choose a writable folder on your shell’s PATH. The default is ~/.local/bin; it will be created if needed. If you move the app, install the command again. Changing folders leaves any previous launcher in place."))
                .when_some(self.cli_message.clone(), |d, message| d.child(div().child(message))))
            .child(div().flex().flex_col().gap_3()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Troubleshooting"))
                .child(div().flex().child(self.button(9+APP_ICONS.len(),"Open diagnostics folder",false,Choice::Diagnostics,d,cx))))
            .child(div().text_sm().text_color(muted).child("Changes save automatically. Fonts, plugins and editing preferences stay in your Neovim configuration."))
            .when_some(self.error.clone(), |d,e| d.child(div().text_color(rgb(0xd75b65)).child(e)))
    }
}

pub fn open(cx: &mut App) {
    refresh(cx);
    for h in cx.windows() {
        if h.downcast::<PreferencesView>().is_some() {
            let _ = h.update(cx, |_, w, _| w.activate_window());
            cx.activate(true);
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(620.), px(720.)), cx);
    if let Err(e) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(560.), px(580.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Zvim Settings".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |w, cx| cx.new(|cx| PreferencesView::new(w, cx)),
    ) {
        eprintln!("Cannot open settings: {e:#}");
    }
    cx.activate(true);
}

#[cfg(test)]
mod shortcut_tests {
    use super::validate_shortcuts;
    #[test]
    fn rejects_conflicts_and_accepts_modified_shortcuts() {
        assert!(validate_shortcuts("ctrl-`", "ctrl-shift-`").is_ok());
        assert!(validate_shortcuts("alt-t", "ctrl-m").is_ok());
        assert!(validate_shortcuts("alt-t", "alt-t").is_err());
        assert!(validate_shortcuts("cmd-q", "ctrl-m").is_err());
        assert!(validate_shortcuts("t", "ctrl-m").is_err());
        assert!(validate_shortcuts("nonsense-key", "ctrl-m").is_err());
    }
}
