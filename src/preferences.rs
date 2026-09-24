use gpui::{prelude::*, *};
use std::sync::Arc;
use zvim::icons::{APP_ICONS, resolve};
use zvim::settings::{Preferences, Theme};

pub struct AppPreferences(pub Preferences);
impl Global for AppPreferences {}

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
}

pub struct PreferencesView {
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
        Self {
            icon_images: APP_ICONS
                .iter()
                .map(|icon| {
                    (
                        icon.id,
                        Arc::new(Image::from_bytes(ImageFormat::Png, icon.png.to_vec())),
                    )
                })
                .collect(),
            focus,
            cli_message: None,
            cli_installing: false,
            buttons: (0..(10 + APP_ICONS.len()) as isize)
                .map(|i| cx.focus_handle().tab_index(i).tab_stop(true))
                .collect(),
            error: Preferences::load()
                .err()
                .map(|e| format!("Cannot read settings: {e}")),
        }
    }
    fn choose(&mut self, choice: Choice, cx: &mut Context<Self>) {
        match choice {
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
                .child(div().text_color(muted).child("Updates terminal colours live. Fonts, shell settings and keybindings use your Ghostty configuration.")))
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
