use gpui::{prelude::*, *};
use std::sync::Arc;
use zvim::icons::{APP_ICONS, resolve};
use zvim::settings::{Preferences, TerminalShortcut, Theme};

pub struct AppPreferences(pub Preferences);
impl Global for AppPreferences {}

pub fn bind_keys(cx: &mut App) {
    cx.clear_key_bindings();
    cx.bind_keys(terminal_bindings(&cx.global::<AppPreferences>().0));
}

fn terminal_bindings(preferences: &Preferences) -> Vec<KeyBinding> {
    use crate::ui::*;
    let defaults = Preferences::default();
    let p = if validate_shortcuts(preferences).is_ok() {
        preferences
    } else {
        &defaults
    };
    let mut bindings = vec![
        KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-,"
            } else {
                "ctrl-shift-,"
            },
            OpenSettings,
            None,
        ),
        KeyBinding::new("cmd-w", Close, Some("ZvimWindow")),
        KeyBinding::new("cmd-shift-w", Close, Some("ZvimWindow")),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-n", NewWindow, None),
        KeyBinding::new("cmd-?", HideTerminal, Some("Terminal")),
        KeyBinding::new("cmd-shift-/", HideTerminal, Some("Terminal")),
        KeyBinding::new("ctrl-j", FocusTerminal, Some("Zvim && Normal")),
    ];
    for shortcut in TerminalShortcut::ALL {
        let context = match shortcut {
            TerminalShortcut::New
            | TerminalShortcut::Close
            | TerminalShortcut::Previous
            | TerminalShortcut::Next => "Terminal",
            _ => "ZvimWindow",
        };
        for key in shortcut_aliases(shortcut.key(p)) {
            bindings.push(match shortcut {
                TerminalShortcut::Toggle => KeyBinding::new(&key, ToggleTerminal, Some(context)),
                TerminalShortcut::Focus => KeyBinding::new(&key, FocusTerminal, Some(context)),
                TerminalShortcut::Maximize => {
                    KeyBinding::new(&key, MaximizeTerminal, Some(context))
                }
                TerminalShortcut::New => KeyBinding::new(&key, NewTerminal, Some(context)),
                TerminalShortcut::Close => KeyBinding::new(&key, CloseTerminal, Some(context)),
                TerminalShortcut::Previous => {
                    KeyBinding::new(&key, PreviousTerminal, Some(context))
                }
                TerminalShortcut::Next => KeyBinding::new(&key, NextTerminal, Some(context)),
            });
        }
    }
    // These aliases appear in the user's normal/visual editor keymap.
    if p.terminal_focus_key == defaults.terminal_focus_key {
        bindings.push(KeyBinding::new(
            "alt-,",
            FocusTerminal,
            Some("Zvim && Normal"),
        ));
    }
    if p.terminal_toggle_key == defaults.terminal_toggle_key {
        bindings.push(KeyBinding::new(
            "alt-.",
            ToggleTerminal,
            Some("Zvim && Normal"),
        ));
    }
    if p.terminal_previous_key == defaults.terminal_previous_key {
        bindings.push(KeyBinding::new(
            "cmd-alt-left",
            PreviousTerminal,
            Some("Terminal"),
        ));
    }
    if p.terminal_next_key == defaults.terminal_next_key {
        bindings.push(KeyBinding::new(
            "cmd-alt-right",
            NextTerminal,
            Some("Terminal"),
        ));
    }
    for index in 0..9 {
        bindings.push(KeyBinding::new(
            &format!("cmd-{}", index + 1),
            ActivateTerminal(index),
            Some("Terminal"),
        ));
    }
    bindings
}

fn canonical_shortcut(value: &str) -> anyhow::Result<String> {
    let mut key = Keystroke::parse(value).map_err(|e| anyhow::anyhow!("{e}"))?;
    if let Some(base) = match key.key.as_str() {
        "<" => Some(","),
        ">" => Some("."),
        "{" => Some("["),
        "}" => Some("]"),
        "?" => Some("/"),
        _ => None,
    } {
        key.key = base.into();
        key.modifiers.shift = true;
    }
    Ok(key.unparse())
}

fn shortcut_aliases(value: &str) -> Vec<String> {
    let canonical = canonical_shortcut(value).expect("validated shortcut");
    let mut keys = vec![canonical.clone()];
    let mut key = Keystroke::parse(&canonical).unwrap();
    if key.modifiers.shift
        && let Some(shifted) = match key.key.as_str() {
            "," => Some("<"),
            "." => Some(">"),
            "[" => Some("{"),
            "]" => Some("}"),
            "/" => Some("?"),
            _ => None,
        }
    {
        key.key = shifted.into();
        keys.push(key.unparse());
        key.modifiers.shift = false;
        keys.push(key.unparse());
    }
    keys
}

fn validate_shortcuts(p: &Preferences) -> anyhow::Result<()> {
    let mut used = std::collections::HashSet::new();
    for shortcut in TerminalShortcut::ALL {
        let value = canonical_shortcut(shortcut.key(p))?;
        let key = Keystroke::parse(&value).unwrap();
        anyhow::ensure!(
            key.modifiers.control || key.modifiers.alt || key.modifiers.platform,
            "Include Control, Option/Alt or Command/Super in the shortcut."
        );
        let mut reserved = vec![
            "cmd-q",
            "cmd-o",
            "cmd-v",
            "cmd-,",
            "ctrl-shift-,",
            "cmd-shift-w",
            "cmd-shift-/",
            "ctrl-j",
            "cmd-1",
            "cmd-2",
            "cmd-3",
            "cmd-4",
            "cmd-5",
            "cmd-6",
            "cmd-7",
            "cmd-8",
            "cmd-9",
        ];
        if shortcut != TerminalShortcut::New {
            reserved.push("cmd-n");
        }
        if shortcut != TerminalShortcut::Close {
            reserved.push("cmd-w");
        }
        if shortcut != TerminalShortcut::Focus {
            reserved.push("alt-,");
        }
        if shortcut != TerminalShortcut::Toggle {
            reserved.push("alt-.");
        }
        if shortcut != TerminalShortcut::Previous {
            reserved.push("cmd-alt-left");
        }
        if shortcut != TerminalShortcut::Next {
            reserved.push("cmd-alt-right");
        }
        for reserved in reserved {
            anyhow::ensure!(
                value != canonical_shortcut(reserved)?,
                "That shortcut is reserved by Zvim."
            );
        }
        anyhow::ensure!(
            used.insert(value),
            "Each terminal action needs a different shortcut."
        );
    }
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    General,
    Keybindings,
}

const GENERAL_TAB_BUTTON: usize = 11 + APP_ICONS.len() + TerminalShortcut::ALL.len();
const KEYBINDINGS_TAB_BUTTON: usize = GENERAL_TAB_BUTTON + 1;

#[derive(Clone, Copy)]
enum Choice {
    Tab(SettingsTab),
    Theme(Theme),
    Geometry,
    Sync,
    TerminalTheme,
    Diagnostics,
    CliDirectory,
    CliDefault,
    CliInstall,
    Icon(&'static str),
    RecordShortcut(TerminalShortcut),
    ResetShortcuts,
}

pub struct PreferencesView {
    tab: SettingsTab,
    recording_shortcut: Option<TerminalShortcut>,
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
            tab: SettingsTab::General,
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
            buttons: (0..=KEYBINDINGS_TAB_BUTTON)
                .map(|i| {
                    cx.focus_handle()
                        .tab_index(if i >= GENERAL_TAB_BUTTON {
                            (i - GENERAL_TAB_BUTTON) as isize
                        } else {
                            i as isize + 2
                        })
                        .tab_stop(true)
                })
                .collect(),
            error: Preferences::load()
                .err()
                .map(|e| format!("Cannot read settings: {e}")),
        }
    }
    fn choose(&mut self, choice: Choice, cx: &mut Context<Self>) {
        match choice {
            Choice::Tab(tab) => {
                self.tab = tab;
                self.recording_shortcut = None;
                self.error = None;
                cx.notify();
                return;
            }
            Choice::RecordShortcut(shortcut) => {
                self.recording_shortcut = Some(shortcut);
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
                    for shortcut in TerminalShortcut::ALL {
                        shortcut.set(&mut p, shortcut.key(&defaults).into());
                    }
                    self.recording_shortcut = None;
                }
                Choice::Tab(_) | Choice::RecordShortcut(_) => unreachable!(),
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
        let Some(shortcut) = self.recording_shortcut else {
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
            shortcut.set(&mut p, canonical_shortcut(&event.keystroke.unparse())?);
            validate_shortcuts(&p)?;
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
            .tab_index(if index >= GENERAL_TAB_BUTTON {
                (index - GENERAL_TAB_BUTTON) as isize
            } else {
                index as isize + 2
            })
            .px_2()
            .h(px(26.))
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .rounded(px(4.))
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
            .focus(|s| s.border_color(rgb(0x8ba9ff)))
            .hover(|s| s.bg(rgb(if is_dark { 0x3c465c } else { 0xe8edfa })))
            .on_click(cx.listener(move |v, _, w, cx| {
                w.focus(&v.buttons[index]);
                v.choose(choice, cx);
            }))
            .on_key_down(cx.listener(move |v, e: &KeyDownEvent, w, cx| {
                if matches!(choice, Choice::Tab(_))
                    && matches!(e.keystroke.key.as_str(), "left" | "right")
                {
                    let (tab, index) = if v.tab == SettingsTab::General {
                        (SettingsTab::Keybindings, KEYBINDINGS_TAB_BUTTON)
                    } else {
                        (SettingsTab::General, GENERAL_TAB_BUTTON)
                    };
                    v.choose(Choice::Tab(tab), cx);
                    w.focus(&v.buttons[index]);
                    cx.stop_propagation();
                } else if matches!(e.keystroke.key.as_str(), "enter" | "space") {
                    v.choose(choice, cx);
                    cx.stop_propagation();
                }
            }))
            .child(label)
    }
    fn toggle(
        &self,
        index: usize,
        enabled: bool,
        choice: Choice,
        is_dark: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div()
            .id(index)
            .track_focus(&self.buttons[index])
            .tab_index(index as isize + 2)
            .relative()
            .w(px(36.))
            .h(px(22.))
            .flex_shrink_0()
            .rounded_full()
            .border_1()
            .border_color(transparent_black())
            .bg(rgb(if enabled {
                0x34c759
            } else if is_dark {
                0x646970
            } else {
                0xadb2b9
            }))
            .cursor_pointer()
            .focus(|s| s.border_color(rgb(0x8ba9ff)))
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
            .child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left(px(if enabled { 16. } else { 2. }))
                    .size(px(16.))
                    .rounded_full()
                    .bg(rgb(0xffffff)),
            )
    }
}
impl Render for PreferencesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.global::<AppPreferences>().0.clone();
        let d = dark(window, cx);
        let muted = rgb(if d { 0xb1b8c6 } else { 0x5d6573 });
        div().size_full().id("settings").p_5().flex().flex_col().gap_4().overflow_hidden()
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
            .child(div().text_xl().flex_shrink_0().font_weight(FontWeight::SEMIBOLD).child("Settings"))
            .child(div().flex().gap_2().flex_shrink_0()
                .child(self.button(GENERAL_TAB_BUTTON, "General", self.tab == SettingsTab::General, Choice::Tab(SettingsTab::General), d, cx))
                .child(self.button(KEYBINDINGS_TAB_BUTTON, "Keybindings", self.tab == SettingsTab::Keybindings, Choice::Tab(SettingsTab::Keybindings), d, cx)))
            .child(div().flex_1().min_h_0().w_full()
                .when(self.tab == SettingsTab::General, |page| page.child(
                    div().id("general-settings").size_full().overflow_y_scroll().flex().flex_col().gap_4()
            .child(div().flex().flex_col().gap_2()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Appearance"))
                .child(div().flex().gap_2()
                    .child(self.button(0,"System",p.theme==Theme::System,Choice::Theme(Theme::System),d,cx))
                    .child(self.button(1,"Light",p.theme==Theme::Light,Choice::Theme(Theme::Light),d,cx))
                    .child(self.button(2,"Dark",p.theme==Theme::Dark,Choice::Theme(Theme::Dark),d,cx)))
                .child(div().text_color(muted).child("System follows your desktop appearance. Native titlebars follow the operating system."))
                .child(div().flex().items_center().justify_between().gap_4()
                    .child("Sync editor appearance")
                    .child(self.toggle(3,p.sync_editor_appearance,Choice::Sync,d,cx)))
                .child(div().text_color(muted).child("Sets Neovim’s light/dark background for compatible colourschemes. NvChad/Base46 palettes stay unchanged. When off, your configuration controls editor colours.")))
            .child(div().border_t_1().border_color(rgb(if d {0x3b4049} else {0xd9dee7})).pt_4().flex().flex_col().gap_2()
                .child(div().flex().items_center().justify_between().gap_4()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Remember window size and position"))
                    .child(self.toggle(4,p.remember_window_geometry,Choice::Geometry,d,cx)))
                .child(div().text_color(muted).child("New windows use the last saved placement. When off, they open centred at the default size. Files and sessions are not restored.")))
            .child(div().flex().flex_col().gap_2()
                .child(div().flex().items_center().justify_between().gap_4()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Follow Neovim terminal colours"))
                    .child(self.toggle(5,p.terminal_follow_neovim,Choice::TerminalTheme,d,cx)))
                .child(div().text_color(muted).child("When enabled, terminal colours follow Neovim live. When disabled, your Ghostty theme is used. Pane shortcuts are in the Keybindings tab.")))
            .child(div().flex().flex_col().gap_2()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Application icon"))
                .child(div().flex().items_center().gap_4()
                    .child(img(self.icon_images[resolve(&p.app_icon).id].clone())
                        .w(px(40.)).h(px(40.)))
                    .children(APP_ICONS.iter().enumerate().map(|(i, icon)|
                        self.button(6+i, icon.label, resolve(&p.app_icon).id == icon.id, Choice::Icon(icon.id), d, cx))))
                .child(div().text_color(muted).child("The Neovim icon is included in this build. More icon choices can be added in future releases.")))
            .child(div().flex().flex_col().gap_2()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Command-line launcher"))
                .child(div().text_color(muted).child("Install the zvim command for this app. No Python or additional tools are needed."))
                .child(div().child(format!("Bin directory: {}", p.cli_bin_directory.display())))
                .child(div().flex().flex_wrap().gap_2()
                    .child(self.button(6+APP_ICONS.len(), "Choose folder…", false, Choice::CliDirectory, d, cx))
                    .child(self.button(7+APP_ICONS.len(), "Use default", false, Choice::CliDefault, d, cx))
                    .child(self.button(8+APP_ICONS.len(), if self.cli_installing {"Installing…"} else {"Install / update zvim"}, false, Choice::CliInstall, d, cx)))
                .child(div().text_color(muted).child("Choose a writable folder on your shell’s PATH. The default is ~/.local/bin; it will be created if needed. If you move the app, install the command again. Changing folders leaves any previous launcher in place."))
                .when_some(self.cli_message.clone(), |d, message| d.child(div().child(message))))
            .child(div().flex().flex_col().gap_2()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Troubleshooting"))
                .child(div().flex().child(self.button(9+APP_ICONS.len(),"Open diagnostics folder",false,Choice::Diagnostics,d,cx))))
                ))
                .when(self.tab == SettingsTab::Keybindings, |page| page.child(
                    div().id("keybinding-settings").size_full().overflow_y_scroll().flex().flex_col().gap_4()
            .child(div().flex().flex_col().gap_2()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Terminal keybindings"))
                .children(TerminalShortcut::ALL.into_iter().enumerate().map(|(i, shortcut)|
                    div().flex().items_center().justify_between().gap_4()
                        .child(format!("{}: {}", shortcut.label(), shortcut.key(&p)))
                        .child(self.button(10+APP_ICONS.len()+i, if self.recording_shortcut == Some(shortcut) {"Press shortcut…"} else {"Change"}, false, Choice::RecordShortcut(shortcut), d, cx))))
                .child(div().flex().child(self.button(10+APP_ICONS.len()+TerminalShortcut::ALL.len(), "Reset shortcuts", false, Choice::ResetShortcuts, d, cx)))
                .child(div().text_color(muted).child("Defaults match your Zed terminal keys. Click Change, then press a shortcut; Escape cancels. New, close and tab-switch shortcuts apply while the terminal is focused. Cmd+1–9 selects a terminal tab.")))
                )))
            .child(div().text_sm().flex_shrink_0().text_color(muted).child("Changes save automatically. Fonts, plugins and editing preferences stay in your Neovim configuration."))
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
    use super::{
        Preferences, canonical_shortcut, shortcut_aliases, terminal_bindings, validate_shortcuts,
    };
    use gpui::{Action, KeyContext, Keymap, Keystroke};
    fn assert_action(map: &Keymap, key: &str, context: &str, expected: &dyn Action) {
        let contexts = [
            KeyContext::parse("ZvimWindow").unwrap(),
            KeyContext::parse(context).unwrap(),
        ];
        let (bindings, pending) =
            map.bindings_for_input(&[Keystroke::parse(key).unwrap()], &contexts);
        assert!(!pending);
        assert!(
            bindings
                .first()
                .is_some_and(|b| b.action().partial_eq(expected)),
            "{key} in {context}"
        );
    }
    #[test]
    fn zed_bindings_respect_terminal_and_editor_contexts() {
        use crate::ui::*;
        let map = Keymap::new(terminal_bindings(&Preferences::default()));
        for key in ["cmd-shift-,", "cmd-<", "cmd-shift-<"] {
            assert_action(&map, key, "Terminal", &FocusTerminal);
            assert_action(&map, key, "Zvim", &FocusTerminal);
        }
        assert_action(&map, "cmd->", "Terminal", &ToggleTerminal);
        assert_action(&map, "cmd-shift-enter", "Terminal", &MaximizeTerminal);
        assert_action(&map, "cmd-n", "Terminal", &NewTerminal);
        assert_action(&map, "cmd-n", "Zvim", &NewWindow);
        assert_action(&map, "cmd-w", "Terminal", &CloseTerminal);
        assert_action(&map, "cmd-w", "Zvim", &Close);
        assert_action(&map, "cmd-shift-w", "Terminal", &Close);
        assert_action(&map, "cmd-{", "Terminal", &PreviousTerminal);
        assert_action(&map, "cmd-}", "Terminal", &NextTerminal);
        assert_action(&map, "cmd-9", "Terminal", &ActivateTerminal(8));
        assert_action(&map, "ctrl-j", "Zvim Normal", &FocusTerminal);
        for (key, context) in [
            ("ctrl-j", "Terminal"),
            ("ctrl-j", "Zvim"),
            ("cmd-1", "Zvim"),
            ("ctrl-`", "Terminal"),
        ] {
            let contexts = [
                KeyContext::parse("ZvimWindow").unwrap(),
                KeyContext::parse(context).unwrap(),
            ];
            assert!(
                map.bindings_for_input(&[Keystroke::parse(key).unwrap()], &contexts)
                    .0
                    .is_empty()
            );
        }
    }
    #[test]
    fn customised_shortcuts_replace_old_bindings_and_aliases() {
        use crate::ui::FocusTerminal;
        let p = Preferences {
            terminal_focus_key: "alt-f".into(),
            ..Preferences::default()
        };
        let map = Keymap::new(terminal_bindings(&p));
        assert_action(&map, "alt-f", "Terminal", &FocusTerminal);
        let contexts = [
            KeyContext::parse("ZvimWindow").unwrap(),
            KeyContext::parse("Terminal").unwrap(),
        ];
        assert!(
            map.bindings_for_input(&[Keystroke::parse("cmd-<").unwrap()], &contexts)
                .0
                .is_empty()
        );
    }
    #[test]
    fn rejects_conflicts_and_accepts_zed_defaults() {
        let mut p = Preferences::default();
        assert!(validate_shortcuts(&p).is_ok());
        p.terminal_toggle_key = "cmd-<".into();
        assert!(validate_shortcuts(&p).is_err());
        p.terminal_toggle_key = "cmd-q".into();
        assert!(validate_shortcuts(&p).is_err());
        p.terminal_toggle_key = "t".into();
        assert!(validate_shortcuts(&p).is_err());
        p.terminal_toggle_key = "alt-t".into();
        assert!(validate_shortcuts(&p).is_ok());
    }
    #[test]
    fn shifted_punctuation_supports_both_platform_event_forms() {
        for (base, shifted) in [
            ("cmd-shift-,", "cmd-<"),
            ("cmd-shift-.", "cmd->"),
            ("cmd-shift-[", "cmd-{"),
        ] {
            assert_eq!(
                canonical_shortcut(base).unwrap(),
                canonical_shortcut(shifted).unwrap()
            );
            assert!(shortcut_aliases(base).contains(&shifted.into()));
        }
    }
}
