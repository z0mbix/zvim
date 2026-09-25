use gpui::{prelude::*, *};
use std::{collections::HashMap, ops::Range, path::PathBuf};
use zvim::{
    grid::{Grid, Highlight},
    input,
    session::{Event, Launch, Session},
    settings::Settings,
    terminal_layout::{Axis, Direction},
};

actions!(
    zvim,
    [
        Open,
        Close,
        Paste,
        NewWindow,
        Quit,
        OpenSettings,
        About,
        ToggleTerminal,
        MaximizeTerminal,
        CloseTerminal,
        FocusTerminal,
        NewTerminal,
        PreviousTerminal,
        NextTerminal,
        HideTerminal,
        SplitTerminalRight,
        SplitTerminalDown,
        TerminalPaneLeft,
        TerminalPaneRight,
        TerminalPaneUp,
        TerminalPaneDown
    ]
);

#[derive(Clone, PartialEq, gpui::Action)]
#[action(no_json)]
pub struct ActivateTerminal(pub usize);

#[derive(Clone, Copy)]
enum CloseTarget {
    Terminal(u64),
    Window,
    Editor,
}

#[derive(Clone, Copy)]
enum TerminalSpawn {
    Tab(Option<u64>),
    Split { pane: u64, axis: Axis },
}

pub struct Editor {
    terminals: zvim::terminal_layout::TerminalLayout<Entity<crate::terminal::Terminal>>,
    terminal_cwd: PathBuf,
    terminal_tab_scroll: HashMap<u64, ScrollHandle>,
    terminal_bounds: Bounds<Pixels>,
    terminal_split_drag: Option<(u64, f32)>,
    pending_terminals: std::collections::VecDeque<TerminalSpawn>,
    terminal_visible: bool,
    terminal_fraction: f32,
    terminal_maximized: bool,
    terminal_drag: Option<(Pixels, f32)>,
    terminal_theme: Option<zvim::terminal_theme::TerminalTheme>,
    terminal_theme_config: zvim::terminal_theme::ThemeConfigCache,
    close_pending: bool,
    close_requested: bool,
    close_restore_terminal_focus: bool,
    exit_confirmed: bool,
    session: Option<Session>,
    grid: Grid,
    focus: FocusHandle,
    error: Option<String>,
    exited: bool,
    bounds: Bounds<Pixels>,
    cell_width: f32,
    cell_height: f32,
    font: Font,
    font_size: f32,
    font_spec: String,
    cache: zvim::text_cache::TextCache<ShapedLine>,
    requested: (usize, usize),
    preedit: String,
    selection: Range<usize>,
    scroll: Point<f32>,
    started: std::time::Instant,
    first_paint: bool,
    font_resolution_ms: u128,
    applied_background: Option<bool>,
    appearance_ready: bool,
    geometry_save_task: Option<Task<()>>,
    persistent_window: bool,
}
impl Editor {
    pub fn new(launch: Launch, window: &mut Window, cx: &mut Context<Self>) -> Self {
        zvim::startup::mark("editor_create");
        let persistent_window = launch.state_directory.is_none();
        let started = std::time::Instant::now();
        let focus = cx.focus_handle();
        window.focus(&focus);
        let session = Session::spawn(&launch);
        zvim::startup::mark("editor_spawned");
        let error = session.as_ref().err().map(|e| format!("{e:#}"));
        let session = session.ok();
        if let Some(s) = &session {
            let events = s.events.clone();
            let rpc = s.rpc.clone();
            let (init_tx, init_rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let _ = init_tx.send_blocking(
                    zvim::session::initialize(&rpc, 100, 35).map_err(|e| format!("{e:#}")),
                );
            });
            cx.spawn_in(window, async move |view, cx| {
                if let Ok(Err(e)) = init_rx.recv().await {
                    let _ = view.update(cx, |v, cx| {
                        v.error = Some(e);
                        cx.notify();
                    });
                }
                while let Ok(event) = events.recv().await {
                    if view
                        .update_in(cx, |v, w, cx| v.event(event, w, cx))
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        }
        cx.observe_global_in::<crate::preferences::AppPreferences>(window, |v, w, cx| {
            v.apply_appearance(w, cx);
            v.sync_terminal_theme(cx);
        })
        .detach();
        cx.observe_window_appearance(window, |v, w, cx| v.apply_appearance(w, cx))
            .detach();
        cx.observe_window_activation(window, |_, _, cx| crate::preferences::refresh(cx))
            .detach();
        cx.observe_window_bounds(window, |view, window, cx| {
            view.geometry_save_task = Some(cx.spawn_in(window, async move |view, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(300))
                    .await;
                let _ = view.update_in(cx, |view, window, cx| view.save(window, cx));
            }));
        })
        .detach();
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            weak.update(cx, |v, cx| {
                v.save(window, cx);
                if (v.exited || v.session.is_none()) && v.terminals.is_empty() {
                    true
                } else {
                    v.request_close(window, cx);
                    false
                }
            })
            .unwrap_or(true)
        });
        Self {
            terminals: Default::default(),
            terminal_cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            terminal_tab_scroll: HashMap::new(),
            terminal_bounds: Bounds::default(),
            terminal_split_drag: None,
            pending_terminals: Default::default(),
            terminal_visible: false,
            terminal_fraction: 0.4,
            terminal_maximized: false,
            terminal_drag: None,
            terminal_theme: None,
            terminal_theme_config: Default::default(),
            close_pending: false,
            close_requested: false,
            close_restore_terminal_focus: false,
            exit_confirmed: false,
            session,
            grid: Grid::default(),
            focus,
            error,
            exited: false,
            bounds: Bounds::default(),
            cell_width: 9.,
            cell_height: 22.,
            font: font(default_font()),
            font_size: 16.,
            font_spec: String::new(),
            cache: zvim::text_cache::TextCache::new(8192),
            requested: (100, 35),
            preedit: String::new(),
            selection: 0..0,
            scroll: point(0., 0.),
            started,
            first_paint: true,
            font_resolution_ms: 0,
            applied_background: None,
            appearance_ready: false,
            geometry_save_task: None,
            persistent_window,
        }
    }
    fn apply_appearance(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.appearance_ready {
            return;
        }
        let p = &cx.global::<crate::preferences::AppPreferences>().0;
        let desired = p
            .sync_editor_appearance
            .then(|| crate::preferences::dark(window, cx));
        if desired != self.applied_background {
            if let Some(s) = &self.session {
                s.set_appearance(desired);
            }
            self.applied_background = desired;
        }
    }
    fn sync_terminal_theme(&mut self, cx: &mut Context<Self>) {
        let _span = zvim::startup::Span::new("theme_sync_begin", "theme_sync_end");
        let theme = cx
            .global::<crate::preferences::AppPreferences>()
            .0
            .terminal_follow_neovim
            .then_some(self.terminal_theme.as_ref())
            .flatten();
        if !self
            .terminal_theme_config
            .update(theme, self.grid.foreground, self.grid.background)
        {
            return;
        }
        for (_, terminal) in self.terminals.iter() {
            let config = self.terminal_theme_config.config.clone();
            if let Err(error) = terminal.update(cx, |terminal, _| terminal.set_theme(config)) {
                self.error = Some(format!("Cannot update terminal theme: {error}"));
                cx.notify();
            }
        }
    }
    fn save(&self, window: &Window, cx: &App) {
        if !self.persistent_window
            || window.is_fullscreen()
            || !cx
                .global::<crate::preferences::AppPreferences>()
                .0
                .remember_window_geometry
        {
            return;
        }
        let bounds = window.window_bounds().get_bounds();
        // GPUI's macOS constructor accepts content dimensions, but window_bounds
        // returns the outer frame, including the native title bar.
        #[cfg(target_os = "macos")]
        let s = window.viewport_size();
        #[cfg(not(target_os = "macos"))]
        let s = bounds.size;
        zvim::geometry_writer::submit(Settings {
            width: f32::from(s.width),
            height: f32::from(s.height),
            x: Some(f32::from(bounds.origin.x)),
            y: Some(f32::from(bounds.origin.y)),
        });
    }
    fn close(&self) {
        if let Some(s) = &self.session {
            s.close();
        }
    }
    fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_pending || self.close_requested {
            return;
        }
        if self.exited || self.session.is_none() {
            self.confirm_terminal_close(CloseTarget::Window, window, cx);
        } else {
            self.confirm_terminal_close(CloseTarget::Editor, window, cx);
        }
    }
    fn hide_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (_, t) in self.terminals.iter() {
            t.update(cx, |t, _| t.set_visible(false));
        }
        self.terminal_visible = false;
        self.terminal_split_drag = None;
        self.terminal_maximized = false;
        self.terminal_drag = None;
        window.focus(&self.focus);
        window.refresh();
        cx.notify();
    }
    fn reveal_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal_visible = true;
        self.sync_terminal_visibility(cx);
        if let Some(t) = self.terminals.active() {
            t.update(cx, |t, cx| t.focus(window, cx));
        }
        window.refresh();
        cx.notify();
    }
    fn toggle_terminal(&mut self, _: &ToggleTerminal, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_pending || self.close_requested {
            return;
        }
        if self.terminal_visible {
            if !self.exited {
                self.hide_terminal(window, cx);
            }
        } else if !self.terminals.is_empty() {
            self.reveal_terminal(window, cx);
        } else {
            self.new_terminal(&NewTerminal, window, cx);
        }
    }
    fn focus_terminal(&mut self, _: &FocusTerminal, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_pending || self.close_requested {
            return;
        }
        if self.terminal_visible
            && self
                .terminals
                .active()
                .is_some_and(|t| t.read(cx).is_focused(window))
        {
            if !self.exited {
                self.terminal_maximized = false;
                self.sync_terminal_visibility(cx);
                window.focus(&self.focus);
                window.refresh();
                cx.notify();
            }
        } else if self.terminals.is_empty() {
            self.new_terminal(&NewTerminal, window, cx);
        } else {
            self.reveal_terminal(window, cx);
        }
    }
    fn select_terminal(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_pending || self.close_requested || index >= self.terminals.len() {
            return;
        }
        if let Some(t) = self.terminals.active() {
            t.update(cx, |t, _| t.set_visible(false));
        }
        self.terminals.select(index);
        self.scroll_terminal_tabs();
        self.reveal_terminal(window, cx);
    }
    fn new_terminal(&mut self, _: &NewTerminal, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_pending || self.close_requested {
            return;
        }
        self.request_terminal(
            TerminalSpawn::Tab(self.terminals.focused_pane()),
            window,
            cx,
        );
    }
    fn request_terminal(
        &mut self,
        target: TerminalSpawn,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_pending || self.close_requested {
            return;
        }
        if let Some(session) = &self.session {
            self.pending_terminals.push_back(target);
            session.new_terminal();
        } else {
            self.spawn_terminal(self.terminal_cwd.clone(), target, window, cx);
        }
    }
    fn split_terminal(&mut self, axis: Axis, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.terminals.focused_pane() {
            self.request_terminal(TerminalSpawn::Split { pane, axis }, window, cx);
        } else {
            self.new_terminal(&NewTerminal, window, cx);
        }
    }
    fn sync_terminal_visibility(&self, cx: &mut Context<Self>) {
        let visible = self.terminals.visible_ids(self.terminal_maximized);
        for (id, terminal) in self.terminals.iter() {
            terminal.update(cx, |terminal, _| {
                terminal.set_visible(self.terminal_visible && visible.contains(id))
            });
        }
    }
    fn scroll_terminal_tabs(&self) {
        if let Some(pane) = self.terminals.focused_pane()
            && let Some(scroll) = self.terminal_tab_scroll.get(&pane)
        {
            scroll.scroll_to_item(self.terminals.active_index());
        }
    }
    fn select_terminal_pane(&mut self, pane: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_pending || self.close_requested {
            return;
        }
        if self.terminals.focus_pane(pane) {
            self.reveal_terminal(window, cx);
        }
    }
    fn navigate_terminal(
        &mut self,
        direction: Direction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let size = self.terminal_bounds.size;
        if let Some(pane) =
            self.terminals
                .neighbour(direction, f32::from(size.width), f32::from(size.height))
        {
            self.select_terminal_pane(pane, window, cx);
        }
    }

    fn maximize_terminal(
        &mut self,
        _: &MaximizeTerminal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exited || self.close_pending || self.close_requested {
            return;
        }
        if !self.terminal_visible {
            self.toggle_terminal(&ToggleTerminal, window, cx);
            self.terminal_maximized = true;
        } else {
            self.terminal_maximized = !self.terminal_maximized;
        }
        self.sync_terminal_visibility(cx);
        if let Some(terminal) = self.terminals.active() {
            terminal.update(cx, |t, cx| t.focus(window, cx));
        }
        window.refresh();
        cx.notify();
    }
    fn show_terminal(&mut self, cwd: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_pending || self.close_requested {
            return;
        }
        if self.terminals.is_empty() {
            self.spawn_terminal(cwd, TerminalSpawn::Tab(None), window, cx);
        } else {
            self.toggle_terminal(&ToggleTerminal, window, cx);
        }
    }
    fn spawn_terminal(
        &mut self,
        cwd: PathBuf,
        target: TerminalSpawn,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_pending || self.close_requested {
            return;
        }
        let target_pane = match target {
            TerminalSpawn::Tab(pane) => pane,
            TerminalSpawn::Split { pane, .. } => Some(pane),
        };
        if target_pane.is_some_and(|pane| self.terminals.pane(pane).is_none()) {
            return;
        }
        let shell = std::env::var("SHELL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/bin/sh".into());
        let command = format!("'{}' -l", shell.replace('\'', "'\\''"));
        let mut options = gpui_libghostty::TerminalOptions::new(command, cwd.clone());
        options.configuration = gpui_libghostty::TerminalConfiguration::UserDefault;
        match crate::terminal::Terminal::spawn(options, window, cx) {
            Ok(t) => {
                let terminal = t.clone();
                let id = match target {
                    TerminalSpawn::Tab(pane) => {
                        if let Some(pane) = pane {
                            self.terminals.focus_pane(pane);
                        }
                        self.terminals.push(t)
                    }
                    TerminalSpawn::Split { pane, axis } => {
                        self.terminal_maximized = false;
                        self.terminals
                            .split(pane, axis, t)
                            .expect("target pane exists")
                    }
                };
                cx.subscribe_in(
                    &terminal,
                    window,
                    move |view, _, _: &crate::terminal::TerminalFocused, _, cx| {
                        if !view.close_pending
                            && view.terminals.active_id() != Some(id)
                            && view.terminals.focus_terminal(id)
                        {
                            view.sync_terminal_visibility(cx);
                            cx.notify();
                        }
                    },
                )
                .detach();
                cx.subscribe_in(
                    &terminal,
                    window,
                    |view, _, _: &crate::terminal::TerminalExited, window, cx| {
                        view.reap_exited_terminals(window, cx);
                    },
                )
                .detach();
                self.terminal_cwd = cwd;
                self.scroll_terminal_tabs();
                self.sync_terminal_theme(cx);
                if let Err(error) = terminal.update(cx, |terminal, _| {
                    terminal.set_theme(self.terminal_theme_config.config.clone())
                }) {
                    self.error = Some(format!("Cannot update terminal theme: {error}"));
                }
                self.reveal_terminal(window, cx);
                window.on_next_frame(|window, _| window.refresh());
            }
            Err(error) => {
                self.error = Some(format!("Cannot open terminal: {error}"));
                cx.notify();
            }
        }
    }
    fn reap_exited_terminals(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.close_pending {
            return false;
        }
        let exited: Vec<_> = self
            .terminals
            .iter()
            .filter(|(_, terminal)| !terminal.read(cx).is_alive())
            .map(|(id, terminal)| (*id, terminal.read(cx).is_focused(window)))
            .collect();
        if exited.is_empty() {
            return false;
        }
        let restore_focus = exited.iter().any(|(_, focused)| *focused);
        for (id, _) in exited {
            if let Some(terminal) = self.terminals.remove(id) {
                terminal.update(cx, |terminal, _| terminal.set_visible(false));
            }
        }
        self.terminal_split_drag = None;
        self.terminal_tab_scroll
            .retain(|pane, _| self.terminals.pane(*pane).is_some());
        if self.terminals.is_empty() {
            self.hide_terminal(window, cx);
            if self.exited {
                window.remove_window();
                return true;
            }
        } else {
            self.scroll_terminal_tabs();
            self.sync_terminal_visibility(cx);
            if restore_focus
                && self.terminal_visible
                && let Some(terminal) = self.terminals.active()
            {
                terminal.update(cx, |terminal, cx| terminal.focus(window, cx));
            }
            window.refresh();
            cx.notify();
        }
        false
    }
    fn close_terminal(&mut self, _: &CloseTerminal, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.terminals.active_id() {
            self.confirm_terminal_close(CloseTarget::Terminal(id), window, cx);
        }
    }
    fn finish_close(&mut self, target: CloseTarget, window: &mut Window, cx: &mut Context<Self>) {
        match target {
            CloseTarget::Editor => {
                self.exit_confirmed = true;
                self.close_requested = true;
                self.close_restore_terminal_focus = self
                    .terminals
                    .active()
                    .is_some_and(|t| t.read(cx).is_focused(window));
                window.focus(&self.focus);
                self.close();
            }
            CloseTarget::Terminal(id) => {
                if !self.terminals.iter().any(|(tab, _)| *tab == id) {
                    return;
                }
                if let Some((_, terminal)) = self.terminals.iter().find(|(tab, _)| *tab == id) {
                    terminal.update(cx, |terminal, _| terminal.set_visible(false));
                }
                self.terminals.remove(id);
                self.terminal_tab_scroll
                    .retain(|pane, _| self.terminals.pane(*pane).is_some());
                if self.terminals.is_empty() {
                    self.hide_terminal(window, cx);
                    if self.exited {
                        window.remove_window();
                    }
                } else {
                    self.scroll_terminal_tabs();
                    self.reveal_terminal(window, cx);
                }
            }
            CloseTarget::Window => {
                self.terminals = Default::default();
                window.remove_window();
            }
        }
    }
    fn confirm_terminal_close(
        &mut self,
        target: CloseTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_pending {
            return;
        }
        let needs_confirmation = self.terminals.iter().any(|(id, t)| {
            let included = match target {
                CloseTarget::Terminal(target) => *id == target,
                _ => true,
            };
            included && t.read(cx).needs_confirm_quit()
        });
        if !needs_confirmation {
            self.finish_close(target, window, cx);
            return;
        }
        let was_focused = self
            .terminals
            .active()
            .is_some_and(|t| t.read(cx).is_focused(window));
        // Composite a still frame while the native surface is hidden, so it cannot
        // paint over the modal and the terminal does not turn into an empty pane.
        let visible = self.terminals.visible_ids(self.terminal_maximized);
        for (id, terminal) in self.terminals.iter() {
            if self.terminal_visible && visible.contains(id) {
                terminal.update(cx, |terminal, cx| terminal.prepare_close_prompt(cx));
            }
        }
        self.close_pending = true;
        cx.notify();
        let closing_editor = !matches!(target, CloseTarget::Terminal(_));
        let answer = window.prompt(
            PromptLevel::Warning,
            if closing_editor { if self.exited { "Close the remaining terminals?" } else { "Close window and stop terminal processes?" } } else { "Stop terminal processes?" },
            Some("Closing a terminal stops its shell and running programs. Closing the window stops every terminal tab, including hidden tabs."),
            &[if self.exited { "Keep Terminal" } else { "Cancel" }, if closing_editor { "Close Window" } else { "Close Terminal" }],
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            let confirmed = answer.await.ok() == Some(1);
            let _ = view.update_in(cx, |v, w, cx| {
                v.close_pending = false;
                if v.reap_exited_terminals(w, cx) {
                    return;
                }
                v.sync_terminal_visibility(cx);
                if let Some(terminal) = v.terminals.active() {
                    terminal.update(cx, |terminal, cx| {
                        terminal.set_visible(v.terminal_visible);
                        if was_focused && v.terminal_visible {
                            terminal.focus(w, cx);
                        }
                    });
                }
                if confirmed {
                    v.finish_close(target, w, cx);
                } else if v.exited
                    && let Some(terminal) = v.terminals.active()
                {
                    let terminal = terminal.clone();
                    v.terminal_visible = true;
                    v.sync_terminal_visibility(cx);
                    terminal.update(cx, |terminal, cx| terminal.focus(w, cx));
                }
                w.refresh();
                cx.notify();
            });
        })
        .detach();
    }
    fn event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            Event::CloseReturned => {
                self.exit_confirmed = false;
                self.close_requested = false;
                if self.close_restore_terminal_focus
                    && self.terminal_visible
                    && let Some(terminal) = self.terminals.active()
                {
                    terminal.update(cx, |terminal, cx| terminal.focus(window, cx));
                }
                self.close_restore_terminal_focus = false;
                cx.notify();
            }
            Event::TerminalTheme(theme) => {
                self.terminal_theme = Some(theme);
                self.sync_terminal_theme(cx);
            }
            Event::Terminal(cwd) => self.show_terminal(cwd, window, cx),
            Event::NewTerminal(cwd) => {
                if let Some(target) = self.pending_terminals.pop_front() {
                    self.spawn_terminal(cwd, target, window, cx);
                }
            }
            Event::Frame => {
                zvim::startup::mark("frame_received");
                if let Some(g) = self.session.as_ref().and_then(Session::take_frame) {
                    self.grid = g;
                    self.appearance_ready = true;
                    self.apply_appearance(window, cx);
                    self.sync_terminal_theme(cx);

                    let title = self.grid.title.trim();
                    window.set_window_title(if title.is_empty() || title == "Zvim" {
                        "Zvim"
                    } else {
                        title
                    });
                    cx.notify();
                }
            }
            Event::ClipboardCopy(text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
            Event::ClipboardPaste(id) => {
                let text = cx
                    .read_from_clipboard()
                    .and_then(|c| c.text())
                    .unwrap_or_default();
                let linewise = text.ends_with('\n');
                let text = if linewise {
                    &text[..text.len() - 1]
                } else {
                    &text
                };
                let lines = rmpv::Value::Array(text.split('\n').map(|s| s.into()).collect());
                if let Some(s) = &self.session {
                    s.rpc.reply(
                        id,
                        rmpv::Value::Array(vec![lines, if linewise { "V" } else { "v" }.into()]),
                    );
                }
            }
            Event::Error(e) => {
                if !self.exited {
                    self.error = Some(e);
                }
                cx.notify();
            }
            Event::Exited(success) => {
                self.exited = true;
                self.close_requested = false;
                self.session = None;
                window.set_window_title("Zvim — Terminal");
                if !self.terminals.is_empty() && !self.exit_confirmed {
                    self.terminal_visible = true;
                }
                self.save(window, cx);
                if success {
                    self.error = None;
                    if self.exit_confirmed {
                        self.finish_close(CloseTarget::Window, window, cx);
                    } else {
                        self.confirm_terminal_close(CloseTarget::Window, window, cx);
                    }
                } else {
                    self.error=Some("Neovim exited unexpectedly. See neovim.log in the Zvim configuration directory.".into());
                    cx.notify();
                }
            }
        }
    }
    fn open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        let result = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: true,
            prompt: Some("Open in Neovim".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            if let Ok(Ok(Some(paths))) = result.await {
                let _ = view.update(cx, |v, _| {
                    if let Some(s) = &v.session {
                        s.open_files(&paths);
                    }
                });
            }
        })
        .detach();
    }
    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(terminal) = self.terminals.active()
            && terminal.read(cx).is_focused(window)
        {
            terminal.update(cx, |t, _| t.paste());
            return;
        }
        if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text())
            && let Some(s) = &self.session
        {
            s.paste(text);
        }
    }
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" && !self.exited {
            self.error = None;
            cx.notify();
        }
        let k = &event.keystroke;
        let m = k.modifiers;
        // Only desktop shortcuts occupy Command on macOS; Ctrl keys remain Neovim's.
        if cfg!(target_os = "macos") && m.platform && !m.control && !m.alt {
            match k.key.as_str() {
                "v" => {
                    self.paste(&Paste, window, cx);
                    cx.stop_propagation();
                    return;
                }
                "o" => {
                    self.open(&Open, window, cx);
                    cx.stop_propagation();
                    return;
                }
                "w" => {
                    self.request_close(window, cx);
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }
        if !self.preedit.is_empty() {
            return;
        }
        // Option-generated printable text on macOS is delivered by the native input method.
        let alt = m.alt
            && !(cfg!(target_os = "macos") && k.key_char.is_some() && !m.control && !m.platform);
        if let Some(keys) = input::special_key(&k.key, m.control, alt, m.shift, m.platform) {
            if let Some(s) = &self.session {
                s.input(keys);
            }
            cx.stop_propagation();
        }
    }
    fn mouse(&self, button: &str, action: &str, pos: Point<Pixels>, mods: Modifiers) {
        if !self.grid.mouse {
            return;
        }
        let row = ((f32::from(pos.y - self.bounds.origin.y) / self.cell_height)
            .floor()
            .max(0.) as usize)
            .min(self.grid.height.saturating_sub(1));
        let col = ((f32::from(pos.x - self.bounds.origin.x) / self.cell_width)
            .floor()
            .max(0.) as usize)
            .min(self.grid.width.saturating_sub(1));
        let mods = format!(
            "{}{}{}{}",
            if mods.control { "C" } else { "" },
            if mods.alt { "A" } else { "" },
            if mods.shift { "S" } else { "" },
            if mods.platform { "D" } else { "" }
        );
        if let Some(s) = &self.session {
            s.rpc.send(
                "nvim_input_mouse",
                vec![
                    button.into(),
                    action.into(),
                    mods.into(),
                    0.into(),
                    (row as u64).into(),
                    (col as u64).into(),
                ],
            );
        }
    }
    fn wheel(&mut self, e: &ScrollWheelEvent, _: &mut Window, _: &mut Context<Self>) {
        let d = e.delta.pixel_delta(px(self.cell_height));
        self.scroll.y += f32::from(d.y) / self.cell_height;
        self.scroll.x += f32::from(d.x) / self.cell_width;
        for horizontal in [false, true] {
            let delta = if horizontal {
                &mut self.scroll.x
            } else {
                &mut self.scroll.y
            };
            let n = delta.trunc() as i32;
            *delta -= n as f32;
            for _ in 0..n.unsigned_abs().min(50) {
                self.mouse(
                    "wheel",
                    if horizontal {
                        if n > 0 { "left" } else { "right" }
                    } else if n > 0 {
                        "up"
                    } else {
                        "down"
                    },
                    e.position,
                    e.modifiers,
                );
            }
        }
    }
    fn layout_grid(&mut self, bounds: Bounds<Pixels>, window: &mut Window) {
        self.bounds = bounds;
        let spec = format!("{}:{}", self.grid.guifont, self.grid.linespace);
        if self.font_spec != spec {
            let font_started = std::time::Instant::now();
            let text_system = window.text_system();
            let (font, size) = resolve_editor_font(&self.grid.guifont, text_system);
            self.font = font;
            self.font_size = size;
            self.font_resolution_ms += font_started.elapsed().as_millis();
            self.font_spec = spec;
            self.cache.clear();
        }
        let run = TextRun {
            len: 1,
            font: self.font.clone(),
            color: rgb(self.grid.foreground).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let sample = window
            .text_system()
            .shape_line("M".into(), px(self.font_size), &[run], None);
        self.cell_width = f32::from(sample.width).max(1.);
        self.cell_height = (self.font_size * 1.4 + self.grid.linespace as f32).max(self.font_size);
        let cols = (f32::from(bounds.size.width) / self.cell_width)
            .floor()
            .max(2.) as usize;
        let rows = (f32::from(bounds.size.height) / self.cell_height)
            .floor()
            .max(2.) as usize;
        if self.requested != (cols, rows) && self.grid.width > 0 {
            self.requested = (cols, rows);
            if let Some(s) = &self.session {
                s.resize(cols, rows);
            }
        }
    }
    fn cell_bounds(&self, row: usize, col: usize, width: usize) -> Bounds<Pixels> {
        Bounds::new(
            self.bounds.origin
                + point(
                    px(col as f32 * self.cell_width),
                    px(row as f32 * self.cell_height),
                ),
            size(px(width as f32 * self.cell_width), px(self.cell_height)),
        )
    }
    fn text(
        &mut self,
        text: &str,
        hl: &Highlight,
        colors: (u32, u32),
        origin: Point<Pixels>,
        window: &mut Window,
        _cx: &mut App,
    ) {
        CellPainter {
            cache: &mut self.cache,
            font: &self.font,
            font_size: self.font_size,
            cell_height: self.cell_height,
        }
        .text(text, hl, colors, origin, window);
    }
    fn paint(&mut self, window: &mut Window, cx: &mut App) {
        let trace_first = self.first_paint && self.grid.width > 0;
        zvim::startup::mark(if trace_first {
            "first_grid_paint_begin"
        } else {
            "grid_paint_begin"
        });
        if self.first_paint && self.grid.width > 0 {
            self.first_paint = false;
            let timing = format!(
                "first_grid_paint_ms={}\nfont_resolution_ms={}\n",
                self.started.elapsed().as_millis(),
                self.font_resolution_ms
            );
            eprint!("{timing}");
            let dir = zvim::settings::data_dir();
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join("startup.log"), timing);
        }
        window.paint_quad(fill(self.bounds, rgb(self.grid.background)));
        let max_rows = self
            .grid
            .height
            .min((f32::from(self.bounds.size.height) / self.cell_height).ceil() as usize);
        let max_cols = self
            .grid
            .width
            .min((f32::from(self.bounds.size.width) / self.cell_width).ceil() as usize);
        // A layer per cell degenerates GPUI's bounds-ordering tree on a regular
        // terminal grid. Use separate, ordered grid-wide background and glyph layers.
        window.paint_layer(self.bounds, |window| {
            // Background pass precedes glyphs so wide glyphs are never covered by continuation cells.
            for r in 0..max_rows {
                for c in 0..max_cols {
                    let cell = &self.grid.cells[r * self.grid.width + c];
                    let h = self
                        .grid
                        .highlights
                        .get(&cell.highlight)
                        .cloned()
                        .unwrap_or_default();
                    let (_, bg, _) = self.grid.colors(&h);
                    if bg != self.grid.background {
                        window.paint_quad(fill(self.cell_bounds(r, c, 1), rgb(bg)));
                    }
                }
            }
        });
        window.paint_layer(self.bounds, |window| {
            let mut painter = CellPainter {
                cache: &mut self.cache,
                font: &self.font,
                font_size: self.font_size,
                cell_height: self.cell_height,
            };
            for r in 0..max_rows {
                for c in 0..max_cols {
                    let cell = &self.grid.cells[r * self.grid.width + c];
                    if cell.text.is_empty() {
                        continue;
                    }
                    let h = self
                        .grid
                        .highlights
                        .get(&cell.highlight)
                        .cloned()
                        .unwrap_or_default();
                    if cell.text == " " && !h.underline && !h.undercurl && !h.strikethrough {
                        continue;
                    }
                    let (fg, _, sp) = self.grid.colors(&h);
                    painter.text(
                        &cell.text,
                        &h,
                        (fg, sp),
                        self.bounds.origin
                            + point(
                                px(self.cell_width * c as f32),
                                px(self.cell_height * r as f32),
                            ),
                        window,
                    );
                }
            }
        });
        if !self.grid.busy && self.grid.cursor.row < max_rows && self.grid.cursor.col < max_cols {
            let cursor = self.grid.cursor.clone();
            let cell = self
                .grid
                .cell(cursor.row, cursor.col)
                .cloned()
                .unwrap_or_default();
            let width = if self
                .grid
                .cell(cursor.row, cursor.col + 1)
                .is_some_and(|c| c.text.is_empty())
            {
                2
            } else {
                1
            };
            let mut rect = self.cell_bounds(cursor.row, cursor.col, width);
            let h = self
                .grid
                .highlights
                .get(&cursor.attr)
                .cloned()
                .unwrap_or_default();
            let (fg, bg, sp) = if cursor.attr == 0 {
                let cell_h = self
                    .grid
                    .highlights
                    .get(&cell.highlight)
                    .cloned()
                    .unwrap_or_default();
                let (fg, bg, sp) = self.grid.colors(&cell_h);
                (bg, fg, sp)
            } else {
                self.grid.colors(&h)
            };
            let ratio = (cursor.percentage / 100.).clamp(0.05, 1.);
            if cursor.shape == "vertical" {
                rect.size.width = px(self.cell_width * ratio);
            } else if cursor.shape == "horizontal" {
                rect.origin.y += px(self.cell_height * (1. - ratio));
                rect.size.height = px(self.cell_height * ratio);
            }
            if self.focus.is_focused(window) {
                window.paint_quad(fill(rect, rgb(bg)));
                if cursor.shape == "block" {
                    self.text(&cell.text, &h, (fg, sp), rect.origin, window, cx);
                }
            }
        }
        if !self.preedit.is_empty() {
            let origin = self
                .cell_bounds(self.grid.cursor.row, self.grid.cursor.col, 1)
                .origin;
            let text = self.preedit.clone();
            window.paint_quad(fill(
                Bounds::new(
                    origin,
                    size(
                        px(self.cell_width * text.chars().count().max(1) as f32),
                        px(self.cell_height),
                    ),
                ),
                rgb(self.grid.background),
            ));
            self.text(
                &text,
                &Highlight {
                    underline: true,
                    ..Default::default()
                },
                (self.grid.foreground, self.grid.foreground),
                origin,
                window,
                cx,
            );
        }
        zvim::startup::mark(if trace_first {
            "first_grid_paint_end"
        } else {
            "grid_paint_end"
        });
    }
}
struct CellPainter<'a> {
    cache: &'a mut zvim::text_cache::TextCache<ShapedLine>,
    font: &'a Font,
    font_size: f32,
    cell_height: f32,
}
impl CellPainter<'_> {
    fn text(
        &mut self,
        text: &str,
        hl: &Highlight,
        colors: (u32, u32),
        origin: Point<Pixels>,
        window: &mut Window,
    ) {
        let (fg, special) = colors;
        let key = zvim::text_cache::TextStyle {
            foreground: fg,
            special,
            bold: hl.bold,
            italic: hl.italic,
            underline: hl.underline,
            undercurl: hl.undercurl,
            strikethrough: hl.strikethrough,
        };
        if let Some(line) = self.cache.get(key, text) {
            paint_cell_line(line, hl, colors, origin, px(self.cell_height), window);
        } else {
            let mut font = (*self.font).clone();
            if hl.bold {
                font.weight = FontWeight::BOLD;
            }
            if hl.italic {
                font.style = FontStyle::Italic;
            }
            let run = TextRun {
                len: text.len(),
                font,
                color: rgb(fg).into(),
                background_color: None,
                underline: if hl.underline || hl.undercurl {
                    Some(UnderlineStyle {
                        thickness: px(1.),
                        color: Some(rgb(special).into()),
                        wavy: hl.undercurl,
                    })
                } else {
                    None
                },
                strikethrough: if hl.strikethrough {
                    Some(StrikethroughStyle {
                        thickness: px(1.),
                        color: Some(rgb(fg).into()),
                    })
                } else {
                    None
                },
            };
            let line = window.text_system().shape_line(
                text.to_owned().into(),
                px(self.font_size),
                &[run],
                None,
            );
            paint_cell_line(&line, hl, colors, origin, px(self.cell_height), window);
            self.cache.insert(key, text, line);
        }
    }
}

/// Paint cached shaped cells inside an already ordered layer, preserving fallback
/// runs, combining glyphs, colour emoji, and text decorations without a layer per cell.
fn paint_cell_line(
    line: &ShapedLine,
    highlight: &Highlight,
    colors: (u32, u32),
    origin: Point<Pixels>,
    height: Pixels,
    window: &mut Window,
) {
    let baseline = (height - line.ascent - line.descent) / 2. + line.ascent;
    for run in &line.runs {
        for glyph in &run.glyphs {
            let position = origin + glyph.position + point(px(0.), baseline);
            if glyph.is_emoji {
                let _ = window.paint_emoji(position, run.font_id, glyph.id, line.font_size);
            } else {
                let _ = window.paint_glyph(
                    position,
                    run.font_id,
                    glyph.id,
                    line.font_size,
                    rgb(colors.0).into(),
                );
            }
        }
    }
    if highlight.underline || highlight.undercurl {
        window.paint_underline(
            origin + point(px(0.), baseline + line.descent * 0.618),
            line.width,
            &UnderlineStyle {
                thickness: px(1.),
                color: Some(rgb(colors.1).into()),
                wavy: highlight.undercurl,
            },
        );
    }
    if highlight.strikethrough {
        window.paint_strikethrough(
            origin + point(px(0.), (line.ascent * 0.5 + baseline) * 0.5),
            line.width,
            &StrikethroughStyle {
                thickness: px(1.),
                color: Some(rgb(colors.0).into()),
            },
        );
    }
}
fn default_font() -> &'static str {
    if cfg!(target_os = "macos") {
        "Menlo"
    } else {
        "monospace"
    }
}
fn resolve_editor_font(spec: &str, text_system: &WindowTextSystem) -> (Font, f32) {
    // Probe only the requested families. Enumerating the entire OS font collection
    // here blocks the first frame, even when guifont is empty.
    let available = spec
        .split(',')
        .filter_map(|entry| {
            let family = entry.split(':').next()?.replace('_', " ");
            if family.is_empty() {
                return None;
            }
            let id = text_system.resolve_font(&font(family.clone()));
            text_system
                .get_font_for_id(id)
                .filter(|resolved| resolved.family.eq_ignore_ascii_case(&family))
                .map(|_| family)
        })
        .collect::<Vec<_>>();
    parse_font(spec, &available)
}
fn parse_font(spec: &str, available: &[String]) -> (Font, f32) {
    let choices: Vec<_> = spec
        .split(',')
        .map(|entry| {
            let mut parts = entry.split(':');
            let family = parts.next().unwrap_or_default().replace('_', " ");
            let options: Vec<_> = parts.collect();
            (family, options)
        })
        .collect();
    let selected = choices
        .iter()
        .find(|(name, _)| available.iter().any(|f| f.eq_ignore_ascii_case(name)));
    let (family, options) = selected
        .map(|(f, o)| (f.as_str(), o.as_slice()))
        .unwrap_or((default_font(), &[]));
    let size = options
        .iter()
        .find_map(|s| s.strip_prefix('h')?.parse::<f32>().ok())
        .filter(|s| s.is_finite())
        .unwrap_or(16.)
        .clamp(6., 96.);
    let mut font = font(family.to_owned());
    if options.contains(&"b") {
        font.weight = FontWeight::BOLD;
    }
    if options.contains(&"i") {
        font.style = FontStyle::Italic;
    }
    let mut fallbacks = choices
        .iter()
        .filter(|(f, _)| f != family && available.iter().any(|a| a.eq_ignore_ascii_case(f)))
        .map(|(f, _)| f.clone())
        .collect::<Vec<_>>();
    if let Some(symbols) = available
        .iter()
        .find(|f| f.contains("Symbols Nerd Font"))
        .or_else(|| {
            available
                .iter()
                .find(|f| f.contains("Nerd Font") && !f.contains("Propo"))
        })
    {
        fallbacks.push(symbols.clone());
    }
    if !fallbacks.is_empty() {
        font.fallbacks = Some(FontFallbacks::from_fonts(fallbacks));
    }
    (font, size)
}

impl Editor {
    fn render_terminal_panes(
        &mut self,
        width: Pixels,
        height: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let _span = zvim::startup::Span::new("terminal_ui_begin", "terminal_ui_end");
        let (panes, dividers) =
            self.terminals
                .layout(f32::from(width), f32::from(height), self.terminal_maximized);
        let view = cx.entity().downgrade();
        let mut content = div().relative().w_full().h(height).flex_shrink_0().child(
            canvas(
                move |bounds, _, cx| {
                    let _ = view.update(cx, |view, _| view.terminal_bounds = bounds);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        );
        for (pane, rect) in panes {
            let scroll = self.terminal_tab_scroll.entry(pane).or_default().clone();
            let tabs = self.terminals.pane(pane).expect("layout leaf exists");
            let focused = self.terminals.focused_pane() == Some(pane);
            let active = tabs.active_id();
            let terminal = tabs.active().cloned();
            let strip = div()
                .flex()
                .items_center()
                .w_full()
                .h(px(24.))
                .flex_shrink_0()
                .bg(rgb(self.grid.background))
                .text_color(rgb(self.grid.foreground))
                .text_size(px(12.))
                .child(
                    div()
                        .id(("terminal-tabs", pane))
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .overflow_x_scroll()
                        .track_scroll(&scroll)
                        .children(tabs.iter().enumerate().map(|(index, (id, terminal))| {
                            let selected = active == Some(*id);
                            let alive = terminal.read(cx).is_alive();
                            div()
                                .id(("terminal-tab", *id))
                                .flex()
                                .flex_shrink_0()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .h(px(24.))
                                .border_b_2()
                                .border_color(if selected {
                                    if focused {
                                        rgb(0x6789ee).into()
                                    } else {
                                        rgb(self.grid.foreground).into()
                                    }
                                } else {
                                    transparent_black()
                                })
                                .cursor_pointer()
                                .child(format!(
                                    "{}: Terminal {}{}",
                                    index + 1,
                                    id,
                                    if alive { "" } else { " · exited" }
                                ))
                                .on_click(cx.listener(move |v, _, w, cx| {
                                    if !v.close_pending && !v.close_requested {
                                        v.terminals.focus_pane(pane);
                                        v.select_terminal(index, w, cx);
                                    }
                                }))
                                .child(
                                    div()
                                        .id(("close-terminal-tab", *id))
                                        .px_1()
                                        .cursor_pointer()
                                        .child("×")
                                        .on_click(cx.listener(move |v, _, w, cx| {
                                            if !v.close_pending && !v.close_requested {
                                                v.terminals.focus_pane(pane);
                                                v.select_terminal(index, w, cx);
                                                v.close_terminal(&CloseTerminal, w, cx);
                                            }
                                            cx.stop_propagation();
                                        })),
                                )
                        })),
                )
                .child(
                    div()
                        .id(("new-terminal-tab", pane))
                        .px_2()
                        .cursor_pointer()
                        .child("+")
                        .on_click(cx.listener(move |v, _, w, cx| {
                            if !v.close_pending && !v.close_requested {
                                v.terminals.focus_pane(pane);
                                v.new_terminal(&NewTerminal, w, cx);
                            }
                        })),
                );
            content = content.child(
                div()
                    .absolute()
                    .left(px(rect.x))
                    .top(px(rect.y))
                    .w(px(rect.width))
                    .h(px(rect.height))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(strip)
                    .when_some(terminal, |el, terminal| {
                        el.child(
                            div()
                                .w_full()
                                .h(px((rect.height - 24.).max(0.)))
                                .child(terminal),
                        )
                    }),
            );
        }
        // Painted last so the invisible grab area receives input over either pane.
        for divider in dividers {
            let r = divider.rect;
            let horizontal = divider.axis == Axis::Horizontal;
            content = content.child(
                div()
                    .absolute()
                    .left(px(r.x))
                    .top(px(r.y))
                    .w(px(r.width))
                    .h(px(r.height))
                    .bg(rgb(0x666675))
                    .child(
                        div()
                            .id(("terminal-split-divider", divider.id))
                            .absolute()
                            .left(px(if horizontal { -3. } else { 0. }))
                            .top(px(if horizontal { 0. } else { -3. }))
                            .w(px(if horizontal { 7. } else { r.width }))
                            .h(px(if horizontal { r.height } else { 7. }))
                            .cursor(if horizontal {
                                CursorStyle::ResizeLeftRight
                            } else {
                                CursorStyle::ResizeUpDown
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |v, event: &MouseDownEvent, _, cx| {
                                    if !v.close_pending && !v.close_requested {
                                        let grab = if horizontal {
                                            f32::from(event.position.x - v.terminal_bounds.origin.x)
                                                - r.x
                                        } else {
                                            f32::from(event.position.y - v.terminal_bounds.origin.y)
                                                - r.y
                                        };
                                        v.terminal_split_drag = Some((divider.id, grab));
                                    }
                                    cx.stop_propagation();
                                }),
                            ),
                    ),
            );
        }
        content.into_any_element()
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _span = zvim::startup::Span::new("editor_render_begin", "editor_render_end");
        let height = window.viewport_size().height;
        let terminal_height = if self.terminal_visible {
            if self.exited || self.terminal_maximized {
                height
            } else {
                px(split_height(f32::from(height), self.terminal_fraction))
            }
        } else {
            px(0.)
        };
        let split = self.terminal_visible && !self.terminal_maximized && !self.exited;
        let drag_view = cx.entity().downgrade();
        let panes = self.terminal_visible.then(|| {
            self.render_terminal_panes(
                window.viewport_size().width,
                (terminal_height - if split { px(1.) } else { px(0.) }).max(px(0.)),
                cx,
            )
        });
        let mut root = div()
            .size_full()
            .relative()
            .overflow_hidden()
            .track_focus(&self.focus)
            .key_context(
                if matches!(self.grid.mode.as_str(), "normal" | "visual" | "select") {
                    "Zvim Normal"
                } else {
                    "Zvim"
                },
            )
            .on_key_down(cx.listener(Self::key))
            .on_scroll_wheel(cx.listener(Self::wheel))
            .on_mouse_move(cx.listener(|v, e: &MouseMoveEvent, _, _| {
                if let Some(b) = e.pressed_button {
                    v.mouse(button_name(b), "drag", e.position, e.modifiers);
                }
            }))
            .on_drop(cx.listener(|v, p: &ExternalPaths, _, _| {
                if let Some(s) = &v.session {
                    s.open_files(p.paths());
                }
            }));
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            root = root
                .on_mouse_down(
                    button,
                    cx.listener(move |v, e: &MouseDownEvent, w, _| {
                        w.focus(&v.focus);
                        v.mouse(button_name(button), "press", e.position, e.modifiers);
                    }),
                )
                .on_mouse_up(
                    button,
                    cx.listener(move |v, e: &MouseUpEvent, _, _| {
                        v.mouse(button_name(button), "release", e.position, e.modifiers)
                    }),
                )
                .on_mouse_up_out(
                    button,
                    cx.listener(move |v, e: &MouseUpEvent, _, _| {
                        v.mouse(button_name(button), "release", e.position, e.modifiers)
                    }),
                );
        }
        let editor = root
            .when(!self.exited, |el| {
                el.child(GridElement {
                    editor: cx.entity(),
                })
            })
            .when(self.exited, |el| {
                el.bg(rgb(0x24242b))
                    .text_color(rgb(0xdddddd))
                    .p_4()
                    .child("Neovim closed · Terminal session retained")
            });
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .key_context("ZvimWindow")
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::toggle_terminal))
            .on_action(cx.listener(Self::maximize_terminal))
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        let view = drag_view.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Capture {
                                return;
                            }
                            let _ = view.update(cx, |v, cx| {
                                if let Some((id, grab)) = v.terminal_split_drag {
                                    if event.pressed_button != Some(MouseButton::Left) {
                                        v.terminal_split_drag = None;
                                    } else {
                                        let bounds = v.terminal_bounds;
                                        let (_, dividers) = v.terminals.layout(
                                            f32::from(bounds.size.width),
                                            f32::from(bounds.size.height),
                                            false,
                                        );
                                        if let Some(divider) = dividers.iter().find(|d| d.id == id)
                                        {
                                            let (position, origin, length) = match divider.axis {
                                                Axis::Horizontal => (
                                                    f32::from(event.position.x - bounds.origin.x),
                                                    divider.parent.x,
                                                    divider.parent.width,
                                                ),
                                                Axis::Vertical => (
                                                    f32::from(event.position.y - bounds.origin.y),
                                                    divider.parent.y,
                                                    divider.parent.height,
                                                ),
                                            };
                                            v.terminals.resize(
                                                id,
                                                (position - origin - grab) / (length - 1.).max(1.),
                                            );
                                            cx.notify();
                                        }
                                        cx.stop_propagation();
                                    }
                                }
                                if let Some((start, fraction)) = v.terminal_drag {
                                    if event.pressed_button != Some(MouseButton::Left) {
                                        v.terminal_drag = None;
                                    } else {
                                        let height = f32::from(window.viewport_size().height);
                                        v.terminal_fraction = split_height(
                                            height,
                                            fraction
                                                + f32::from(start - event.position.y)
                                                    / height.max(1.),
                                        ) / height.max(1.);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }
                                }
                            });
                        });
                        let view = drag_view.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture && event.button == MouseButton::Left
                            {
                                let _ = view.update(cx, |v, cx| {
                                    let split_drag = v.terminal_split_drag.take().is_some();
                                    if v.terminal_drag.take().is_some() || split_drag {
                                        cx.stop_propagation();
                                        cx.notify();
                                    }
                                });
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
            .on_action(cx.listener(Self::close_terminal))
            .on_action(cx.listener(Self::new_terminal))
            .on_action(cx.listener(Self::focus_terminal))
            .on_action(cx.listener(|v, _: &SplitTerminalRight, w, cx| {
                v.split_terminal(Axis::Horizontal, w, cx)
            }))
            .on_action(cx.listener(|v, _: &SplitTerminalDown, w, cx| {
                v.split_terminal(Axis::Vertical, w, cx)
            }))
            .on_action(cx.listener(|v, _: &TerminalPaneLeft, w, cx| {
                v.navigate_terminal(Direction::Left, w, cx)
            }))
            .on_action(cx.listener(|v, _: &TerminalPaneRight, w, cx| {
                v.navigate_terminal(Direction::Right, w, cx)
            }))
            .on_action(
                cx.listener(|v, _: &TerminalPaneUp, w, cx| {
                    v.navigate_terminal(Direction::Up, w, cx)
                }),
            )
            .on_action(cx.listener(|v, _: &TerminalPaneDown, w, cx| {
                v.navigate_terminal(Direction::Down, w, cx)
            }))
            .on_action(cx.listener(|v, _: &HideTerminal, w, cx| {
                if !v.exited && !v.close_pending && !v.close_requested {
                    v.hide_terminal(w, cx);
                }
            }))
            .on_action(cx.listener(|v, _: &PreviousTerminal, w, cx| {
                v.select_terminal(v.terminals.adjacent(false), w, cx)
            }))
            .on_action(cx.listener(|v, _: &NextTerminal, w, cx| {
                v.select_terminal(v.terminals.adjacent(true), w, cx)
            }))
            .on_action(
                cx.listener(|v, action: &ActivateTerminal, w, cx| {
                    v.select_terminal(action.0, w, cx)
                }),
            )
            .on_action(cx.listener(|v, _: &Close, w, cx| v.request_close(w, cx)))
            .when(terminal_height < height, |root| {
                root.child(
                    div()
                        .w_full()
                        .h(height - terminal_height)
                        .flex_shrink_0()
                        .child(editor),
                )
            })
            .when(self.terminal_visible, |root| {
                root.child(
                    div()
                        .w_full()
                        .h(terminal_height)
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .when(split, |pane| {
                            pane.child(
                                div()
                                    .relative()
                                    .w_full()
                                    .h(px(1.))
                                    .flex_shrink_0()
                                    .bg(rgb(0x666675))
                                    .child(
                                        div()
                                            .id("terminal-divider")
                                            .absolute()
                                            .top(px(-3.))
                                            .w_full()
                                            .h(px(7.))
                                            .cursor(CursorStyle::ResizeUpDown)
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |v, e: &MouseDownEvent, _, cx| {
                                                    if !v.close_pending {
                                                        v.terminal_drag = Some((
                                                            e.position.y,
                                                            f32::from(terminal_height)
                                                                / f32::from(height).max(1.),
                                                        ));
                                                    }
                                                    cx.stop_propagation();
                                                }),
                                            ),
                                    ),
                            )
                        })
                        .children(panes),
                )
            })
            .when_some(self.error.clone(), |el, error| {
                el.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .p_4()
                        .bg(rgb(0x52252c))
                        .text_color(rgb(0xffffff))
                        .child(error),
                )
            })
    }
}
fn split_height(height: f32, fraction: f32) -> f32 {
    let minimum = 80_f32.min(height / 2.);
    (height * fraction).clamp(minimum, (height - minimum).max(minimum))
}

fn button_name(b: MouseButton) -> &'static str {
    match b {
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
        _ => "left",
    }
}
struct GridElement {
    editor: Entity<Editor>,
}
impl IntoElement for GridElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for GridElement {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        w: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (w.request_layout(style, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        w: &mut Window,
        cx: &mut App,
    ) {
        self.editor.update(cx, |v, _| v.layout_grid(bounds, w));
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        w: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.editor.read(cx).focus.clone();
        w.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        self.editor.update(cx, |v, cx| v.paint(w, cx));
    }
}
impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let units = self.preedit.encode_utf16().collect::<Vec<_>>();
        let start = r.start.min(units.len());
        let end = r.end.min(units.len()).max(start);
        *actual = Some(start..end);
        Some(String::from_utf16_lossy(&units[start..end]))
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.selection.clone(),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        if self.preedit.is_empty() {
            None
        } else {
            Some(0..self.preedit.encode_utf16().count())
        }
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.preedit.clear();
        self.selection = 0..0;
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preedit.clear();
        self.selection = 0..0;
        if let Some(s) = &self.session {
            s.input(input::literal(text));
        }
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut units = self.preedit.encode_utf16().collect::<Vec<_>>();
        let r = range.unwrap_or(0..units.len());
        let start = r.start.min(units.len());
        let end = r.end.min(units.len()).max(start);
        units.splice(start..end, text.encode_utf16());
        self.preedit = String::from_utf16_lossy(&units);
        let n = self.preedit.encode_utf16().count();
        self.selection = selected
            .map(|r| (start + r.start).min(n)..(start + r.end).min(n))
            .unwrap_or(n..n);
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(self.cell_bounds(self.grid.cursor.row, self.grid.cursor.col, 1))
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}

pub fn open_window(launch: Launch, cx: &mut App) {
    zvim::startup::mark("window_open_begin");
    crate::preferences::refresh(cx);
    let settings = if cx
        .global::<crate::preferences::AppPreferences>()
        .0
        .remember_window_geometry
    {
        Settings::load()
    } else {
        Settings::default()
    };
    let mut bounds = Bounds::centered(
        None,
        size(
            px(settings.width.clamp(400., 4000.)),
            px(settings.height.clamp(300., 2400.)),
        ),
        cx,
    );
    if let (Some(x), Some(y)) = (settings.x, settings.y) {
        let candidate = Bounds::new(point(px(x), px(y)), bounds.size);
        if x.is_finite()
            && y.is_finite()
            && cx
                .displays()
                .iter()
                .any(|d| d.bounds().contains(&candidate.origin))
        {
            bounds = candidate;
        }
    }
    if let Err(e) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            app_id: Some("zvim".into()),
            window_min_size: Some(size(px(400.), px(300.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Zvim".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |w, cx| {
            zvim::startup::mark("native_window_ready");
            cx.new(|cx| Editor::new(launch, w, cx))
        },
    ) {
        eprintln!("Cannot open Zvim window: {e:#}");
    }
    zvim::startup::mark("window_open_end");
    cx.activate(true);
}
pub fn open_paths(paths: Vec<PathBuf>, cx: &mut App) {
    if let Some(window) = cx
        .windows()
        .into_iter()
        .find(|h| h.downcast::<Editor>().is_some())
    {
        let _ = window.update(cx, |root, _, cx| {
            if let Ok(editor) = root.downcast::<Editor>() {
                editor.update(cx, |v, _| {
                    if let Some(s) = &v.session {
                        s.open_files(&paths);
                    }
                });
            }
        });
    } else {
        open_window(
            Launch {
                files: paths,
                ..Default::default()
            },
            cx,
        );
    }
}

/// Request each session to quit directly; dispatching a focused action can miss inactive windows.
pub fn request_close_all(cx: &mut App) {
    for handle in cx.windows() {
        let _ = handle.update(cx, |root, window, cx| {
            if root
                .clone()
                .downcast::<crate::preferences::PreferencesView>()
                .is_ok()
                || root.clone().downcast::<crate::about::AboutView>().is_ok()
            {
                window.remove_window();
            } else if let Ok(editor) = root.downcast::<Editor>() {
                editor.update(cx, |view, cx| {
                    view.save(window, cx);
                    view.request_close(window, cx);
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{FontStyle, FontWeight, parse_font, split_height};
    #[test]
    fn terminal_split_keeps_both_panes_visible() {
        assert_eq!(split_height(800., 0.4), 320.);
        assert_eq!(split_height(800., -1.), 80.);
        assert_eq!(split_height(800., 2.), 720.);
        assert_eq!(split_height(100., 0.9), 50.);
        assert_eq!(split_height(0., 0.4), 0.);
    }
    #[test]
    fn chooses_installed_font_in_neovim_preference_list() {
        let (font, size) = parse_font("Missing Font,Menlo:h18,monospace", &["Menlo".into()]);
        assert_eq!(font.family.as_ref(), "Menlo");
        assert_eq!(size, 18.);
    }
    #[test]
    fn applies_font_style_and_rejects_nonfinite_size() {
        let (font, size) = parse_font("Example_Mono:hNaN:b:i", &["Example Mono".into()]);
        assert_eq!(font.family.as_ref(), "Example Mono");
        assert_eq!(size, 16.);
        assert_eq!(font.weight, FontWeight::BOLD);
        assert_eq!(font.style, FontStyle::Italic);
    }
}
