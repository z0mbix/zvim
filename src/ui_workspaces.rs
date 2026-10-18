use super::*;
use zvim::terminal_layout::TerminalLayout;
use zvim::terminal_workspace::{SavedTerminal, SavedWorkspace};

pub(super) struct Workspace {
    pub terminals: TerminalLayout<Entity<crate::terminal::Terminal>>,
    cwd: PathBuf,
    scroll: HashMap<u64, ScrollHandle>,
    visible: bool,
    fraction: f32,
    maximized: bool,
    restore_failed: bool,
}

impl Workspace {
    fn snapshot(&self, cx: &App) -> SavedWorkspace {
        SavedWorkspace {
            layout: self.terminals.snapshot(|terminal| SavedTerminal {
                cwd: terminal.read(cx).working_directory().to_owned(),
            }),
            visible: self.visible,
            fraction: self.fraction,
            maximized: self.maximized,
        }
    }
}

impl Editor {
    pub(super) fn render_workspace_bar(&self, cx: &mut Context<Self>) -> Div {
        let name = self
            .project_root
            .as_ref()
            .map(|root| {
                root.file_name()
                    .unwrap_or(root.as_os_str())
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_default();
        let background = self
            .workspaces
            .values()
            .map(|workspace| workspace.terminals.iter().count())
            .sum::<usize>();
        let mut bar = div()
            .w_full()
            .h(px(24.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .text_xs()
            .bg(rgb(0x272b33))
            .text_color(rgb(0xc5cbd5))
            .child(format!("Terminals · {name}"));
        if background > 0 {
            bar = bar.child(format!(
                "{background} background terminal{}",
                if background == 1 { "" } else { "s" }
            ));
        }
        if self.exited {
            let mut roots: Vec<_> = self
                .workspaces
                .iter()
                .filter(|(_, workspace)| !workspace.terminals.is_empty())
                .map(|(root, _)| root.clone())
                .collect();
            roots.sort();
            for (index, root) in roots.into_iter().enumerate() {
                let label = root
                    .file_name()
                    .unwrap_or(root.as_os_str())
                    .to_string_lossy()
                    .into_owned();
                bar = bar.child(
                    div()
                        .id(("background-project", index))
                        .cursor_pointer()
                        .child(label)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.activate_terminal_project(root.clone(), window, cx);
                            view.reveal_terminal(window, cx);
                        })),
                );
            }
        }
        bar
    }

    pub(super) fn all_terminals(
        &self,
    ) -> impl Iterator<Item = &(u64, Entity<crate::terminal::Terminal>)> {
        self.terminals.iter().chain(
            self.workspaces
                .values()
                .flat_map(|workspace| workspace.terminals.iter()),
        )
    }

    pub(super) fn save_terminal_layouts(&self, cx: &App) {
        if !self.workspace_mode
            || !cx
                .global::<crate::preferences::AppPreferences>()
                .0
                .remember_terminal_layouts
        {
            return;
        }
        if let Some(root) = &self.project_root
            && !self.workspace_restore_failed
        {
            zvim::terminal_workspace::save_in(
                self.workspace_directory.clone(),
                root.clone(),
                self.workspace_snapshot(cx),
            );
        }
        for (root, workspace) in &self.workspaces {
            if !workspace.restore_failed {
                zvim::terminal_workspace::save_in(
                    self.workspace_directory.clone(),
                    root.clone(),
                    workspace.snapshot(cx),
                );
            }
        }
    }

    pub(super) fn workspace_snapshot(&self, cx: &App) -> SavedWorkspace {
        SavedWorkspace {
            layout: self.terminals.snapshot(|terminal| SavedTerminal {
                cwd: terminal.read(cx).working_directory().to_owned(),
            }),
            visible: self.terminal_visible,
            fraction: self.terminal_fraction,
            maximized: self.terminal_maximized,
        }
    }

    pub(super) fn update_workspace_preference(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.workspace_allowed
            && !self.workspace_mode
            && cx
                .global::<crate::preferences::AppPreferences>()
                .0
                .remember_terminal_layouts
        {
            self.workspace_mode = true;
            self.project_root = None;
            self.switch_reported_project(window, cx);
        }
    }

    pub(super) fn switch_reported_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.reported_project.clone() else {
            return;
        };
        if !self.workspace_mode {
            self.project_root = Some(root);
            return;
        }
        self.activate_terminal_project(root, window, cx);
    }

    pub(super) fn activate_terminal_project(
        &mut self,
        root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_pending || self.close_requested || self.project_root.as_ref() == Some(&root) {
            return;
        }
        self.save_terminal_layouts(cx);
        for (_, terminal) in self.terminals.iter() {
            terminal.update(cx, |terminal, _| terminal.set_visible(false));
        }
        // A first project notification adopts terminals created before startup finished.
        if self.project_root.is_none() && !self.terminals.is_empty() {
            self.project_root = Some(root);
            self.sync_terminal_visibility(cx);
            return;
        }
        let old = Workspace {
            terminals: std::mem::take(&mut self.terminals),
            cwd: std::mem::replace(&mut self.terminal_cwd, root.clone()),
            scroll: std::mem::take(&mut self.terminal_tab_scroll),
            visible: std::mem::replace(&mut self.terminal_visible, false),
            fraction: std::mem::replace(&mut self.terminal_fraction, 0.4),
            maximized: std::mem::replace(&mut self.terminal_maximized, false),
            restore_failed: std::mem::replace(&mut self.workspace_restore_failed, false),
        };
        if let Some(previous) = self.project_root.replace(root.clone()) {
            self.workspaces.insert(previous, old);
            self.workspace_generation += 1;
        }
        self.terminal_drag = None;
        self.terminal_split_drag = None;
        self.terminal_hover_regions.clear();
        self.terminal_hover_exclusions.clear();
        if let Some(workspace) = self.workspaces.remove(&root) {
            self.terminals = workspace.terminals;
            self.terminal_cwd = workspace.cwd;
            self.terminal_tab_scroll = workspace.scroll;
            self.terminal_visible = workspace.visible;
            self.terminal_fraction = workspace.fraction;
            self.terminal_maximized = workspace.maximized;
            self.workspace_restore_failed = workspace.restore_failed;
        } else if cx
            .global::<crate::preferences::AppPreferences>()
            .0
            .remember_terminal_layouts
        {
            match zvim::terminal_workspace::load_from(&self.workspace_directory, &root) {
                Ok(Some(saved)) => self.restore_terminal_workspace(saved, &root, window, cx),
                Ok(None) => {}
                Err(error) => {
                    self.workspace_restore_failed = true;
                    self.error = Some(format!("Cannot restore terminal layout: {error:#}"));
                }
            }
        }
        let terminals: Vec<_> = self
            .terminals
            .iter()
            .map(|(_, terminal)| terminal.clone())
            .collect();
        for terminal in terminals {
            let config = self.terminal_theme_config.config.clone();
            let _ = terminal.update(cx, |terminal, _| terminal.set_theme(config));
        }
        self.sync_terminal_visibility(cx);
        self.scroll_terminal_tabs();
        if self.terminal_visible && (self.terminal_maximized || self.exited) {
            if let Some(terminal) = self.terminals.active() {
                terminal.update(cx, |terminal, cx| terminal.focus(window, cx));
            }
        } else {
            window.focus(&self.focus);
        }
        window.refresh();
        cx.notify();
    }

    fn restore_terminal_workspace(
        &mut self,
        saved: SavedWorkspace,
        root: &std::path::Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut fallback = false;
        let restored = TerminalLayout::restore(saved.layout, |state| {
            let cwd = if state.cwd.is_absolute() && state.cwd.is_dir() {
                state.cwd
            } else {
                fallback = true;
                root.to_owned()
            };
            self.create_terminal(cwd, false, window, cx)
                .map_err(anyhow::Error::msg)
        });
        match restored {
            Ok(layout) => {
                self.terminals = layout;
                self.terminal_visible = saved.visible && !self.terminals.is_empty();
                self.terminal_fraction = saved.fraction;
                self.terminal_maximized = saved.maximized;
                let terminals: Vec<_> = self
                    .terminals
                    .iter()
                    .map(|(id, terminal)| (*id, terminal.clone()))
                    .collect();
                for (id, terminal) in terminals {
                    self.subscribe_terminal(id, &terminal, window, cx);
                }
                if fallback {
                    self.error = Some("A saved terminal directory is missing; opened its shell in the project root.".into());
                }
            }
            Err(error) => {
                self.workspace_restore_failed = true;
                self.error = Some(format!("Cannot restore terminal shells: {error:#}"));
            }
        }
    }

    pub(super) fn create_terminal(
        &self,
        cwd: PathBuf,
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Entity<crate::terminal::Terminal>, String> {
        let shell = std::env::var("SHELL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/bin/sh".into());
        let command = format!("'{}' -l", shell.replace('\'', "'\\''"));
        let mut options = gpui_libghostty::TerminalOptions::new(command, cwd);
        options.configuration = gpui_libghostty::TerminalConfiguration::UserDefault;
        options.focus_on_spawn = focus;
        crate::terminal::Terminal::spawn(options, window, cx)
    }

    pub(super) fn subscribe_terminal(
        &mut self,
        id: u64,
        terminal: &Entity<crate::terminal::Terminal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entity = terminal.entity_id();
        cx.subscribe_in(
            terminal,
            window,
            move |view, _, _: &crate::terminal::TerminalFocused, _, cx| {
                // IDs are scoped to a project; a delayed event must also match its entity.
                let belongs = view
                    .terminals
                    .iter()
                    .any(|(tab, terminal)| *tab == id && terminal.entity_id() == entity);
                if belongs
                    && !view.close_pending
                    && !view.close_requested
                    && view.terminals.active_id() != Some(id)
                    && view.terminals.focus_terminal(id)
                {
                    view.sync_terminal_visibility(cx);
                    view.save_terminal_layouts(cx);
                    cx.notify();
                }
            },
        )
        .detach();
        cx.subscribe_in(
            terminal,
            window,
            |view, _, _: &crate::terminal::TerminalExited, window, cx| {
                view.reap_exited_terminals(window, cx);
            },
        )
        .detach();
        cx.subscribe_in(
            terminal,
            window,
            |view, _, _: &crate::terminal::TerminalDirectoryChanged, _, cx| {
                view.save_terminal_layouts(cx);
            },
        )
        .detach();
    }

    pub(super) fn reap_background_terminals(&mut self, cx: &mut Context<Self>) {
        for workspace in self.workspaces.values_mut() {
            let exited: Vec<_> = workspace
                .terminals
                .iter()
                .filter(|(_, terminal)| !terminal.read(cx).is_alive())
                .map(|(id, _)| *id)
                .collect();
            for id in exited {
                workspace.terminals.remove(id);
            }
            if workspace.terminals.is_empty() {
                workspace.visible = false;
                workspace.maximized = false;
            }
        }
    }

    pub(super) fn reveal_remaining_project(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.terminals.is_empty() {
            return true;
        }
        let root = self
            .workspaces
            .iter()
            .find(|(_, workspace)| !workspace.terminals.is_empty())
            .map(|(root, _)| root.clone());
        if let Some(root) = root {
            self.activate_terminal_project(root, window, cx);
            self.terminal_visible = true;
            self.sync_terminal_visibility(cx);
            return true;
        }
        false
    }
}
