//! Isolated real-window regression for project switching and terminal layout restoration.
#![allow(dead_code)]
#[path = "../src/about.rs"]
mod about;
#[path = "../src/preferences.rs"]
mod preferences;
#[path = "../src/terminal_search.rs"]
mod terminal_search;
mod terminal {
    include!("../src/terminal.rs");
    impl Terminal {
        pub fn probe_input(&mut self, text: &str) {
            self.surface.text(&std::ffi::CString::new(text).unwrap());
            self.surface
                .send_key(KeyAction::Press, "enter", None, Modifiers::empty());
            self.surface
                .send_key(KeyAction::Release, "enter", None, Modifiers::empty());
        }
    }
}
mod ui {
    include!("../src/ui.rs");

    pub fn probe() {
        use std::time::Duration;
        let state = tempfile::tempdir().unwrap();
        let a = state.path().join("project-a");
        let b = state.path().join("project-b");
        let nested = a.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir(&b).unwrap();
        let a = a.canonicalize().unwrap();
        let b = b.canonicalize().unwrap();
        let nested = nested.canonicalize().unwrap();
        Application::new().run(move |cx| {
            cx.set_global(crate::preferences::AppPreferences(zvim::settings::Preferences {
                remember_terminal_layouts: true,
                ..Default::default()
            }));
            let launch = Launch {
                state_directory: Some(state.path().join("state")),
                working_directory: Some(a.clone()),
                nvim_args: vec!["-u".into(), "NONE".into(), "-i".into(), "NONE".into(), "-n".into()],
                ..Default::default()
            };
            let original_launch = launch.clone();
            let window = cx.open_window(WindowOptions::default(), |window, cx| cx.new(|cx| Editor::new(launch, window, cx))).unwrap();
            cx.activate(true);
            cx.spawn(async move |cx| {
                let _state = state;
                cx.background_executor().timer(Duration::from_secs(2)).await;
                window.update(cx, |view, window, cx| {
                    assert_eq!(view.project_root.as_ref(), Some(&a));
                    view.new_terminal(&NewTerminal, window, cx);
                }).unwrap();
                cx.background_executor().timer(Duration::from_millis(600)).await;
                window.update(cx, |view, window, cx| view.split_terminal(Axis::Horizontal, window, cx)).unwrap();
                cx.background_executor().timer(Duration::from_millis(600)).await;
                let pidfile = _state.path().join("shell-pid");
                window.update(cx, |view, _, cx| {
                    assert_eq!(view.terminals.iter().count(), 2);
                    view.terminal_fraction = 0.55;
                    let command = format!("cd '{}'; printf '\\033]7;file://localhost%s\\007' \"$PWD\"; printf '%s' \"$$\" > '{}'; sleep 45", nested.display(), pidfile.display());
                    view.terminals.active().unwrap().update(cx, |terminal, _| terminal.probe_input(&command));
                }).unwrap();
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let original_pid: i32 = std::fs::read_to_string(&pidfile).unwrap().parse().unwrap();
                let original_entities = window.read_with(cx, |view, cx| {
                    assert_eq!(view.terminals.active().unwrap().read(cx).working_directory(), nested);
                    view.terminals.iter().map(|(_, terminal)| terminal.entity_id()).collect::<Vec<_>>()
                }).unwrap();
                window.update(cx, |view, _, _| {
                    view.session.as_ref().unwrap().rpc.send("nvim_exec_lua", vec!["local root = ...; vim.cmd.cd(vim.fn.fnameescape(root)); require('zvim').set_project(root)".into(), rmpv::Value::Array(vec![b.to_str().unwrap().into()])]);
                }).unwrap();
                cx.background_executor().timer(Duration::from_millis(400)).await;
                window.update(cx, |view, window, cx| {
                    assert_eq!(view.project_root.as_ref(), Some(&b));
                    assert!(view.terminals.is_empty());
                    assert_eq!(view.workspaces[&a].terminals.iter().count(), 2);
                    assert!(view.terminal_close_needs_confirmation(CloseTarget::Editor, cx), "hidden running command must be included in close checks");
                    view.new_terminal(&NewTerminal, window, cx);
                }).unwrap();
                cx.background_executor().timer(Duration::from_millis(600)).await;
                window.update(cx, |view, _, _| {
                    view.close_pending = true;
                    view.session.as_ref().unwrap().rpc.send("nvim_exec_lua", vec!["local root = ...; vim.cmd.cd(vim.fn.fnameescape(root)); require('zvim').set_project(root)".into(), rmpv::Value::Array(vec![a.to_str().unwrap().into()])]);
                }).unwrap();
                cx.background_executor().timer(Duration::from_millis(400)).await;
                window.update(cx, |view, window, cx| {
                    assert_eq!(view.reported_project.as_ref(), Some(&a));
                    assert_eq!(view.project_root.as_ref(), Some(&b), "project switch waits until close prompt resolves");
                    view.close_pending = false;
                    view.switch_reported_project(window, cx);
                    assert_eq!(view.terminals.iter().map(|(_, terminal)| terminal.entity_id()).collect::<Vec<_>>(), original_entities);
                    assert_eq!(view.terminal_fraction, 0.55);
                    // SAFETY: signal 0 only checks existence of the test shell PID.
                    assert_eq!(unsafe { libc::kill(original_pid, 0) }, 0, "original shell must survive project switches");
                    view.save_terminal_layouts(cx);
                }).unwrap();
                cx.background_executor().spawn(async { zvim::terminal_workspace::flush(); }).await;
                let saved = window.read_with(cx, |view, cx| view.workspace_snapshot(cx)).unwrap();
                window.update(cx, |view, window, cx| view.finish_close(CloseTarget::Window, window, cx)).unwrap();
                cx.background_executor().timer(Duration::from_millis(300)).await;
                let recovery_launch = original_launch.clone();
                let reopened = cx.update(|cx| cx.open_window(WindowOptions::default(), |window, cx| cx.new(|cx| Editor::new(original_launch, window, cx))).unwrap()).unwrap();
                cx.background_executor().timer(Duration::from_secs(2)).await;
                reopened.update(cx, |view, _, cx| {
                    assert_eq!(view.project_root.as_ref(), Some(&a));
                    assert_eq!(view.workspace_snapshot(cx), saved, "relaunch must restore topology, focus, sizes and shell directories");
                    assert_eq!(view.terminals.iter().count(), 2);
                    assert_eq!(view.terminals.active().unwrap().read(cx).working_directory(), nested);
                    view.terminals.active().unwrap().update(cx, |terminal, _| terminal.probe_input(&format!("printf '%s' \"$$\" > '{}'", pidfile.display())));
                }).unwrap();
                cx.background_executor().timer(Duration::from_millis(600)).await;
                let fresh_pid: i32 = std::fs::read_to_string(&pidfile).unwrap().parse().unwrap();
                assert_ne!(fresh_pid, original_pid, "relaunch starts a fresh shell");
                reopened.update(cx, |view, window, cx| {
                    view.event(Event::Project(b.clone()), window, cx);
                    assert_eq!(view.terminals.iter().count(), 1, "other project's saved workspace restores on first visit");
                    view.terminals.active().unwrap().update(cx, |terminal, _| terminal.probe_input("exit"));
                    view.event(Event::Project(a.clone()), window, cx);
                }).unwrap();
                cx.background_executor().timer(Duration::from_millis(600)).await;
                reopened.update(cx, |view, window, cx| {
                    assert!(view.workspaces[&b].terminals.is_empty(), "hidden shells are reaped when they exit");
                    view.finish_close(CloseTarget::Window, window, cx);
                }).unwrap();
                cx.background_executor().spawn(async { zvim::terminal_workspace::flush(); }).await;
                std::fs::remove_dir(&nested).unwrap();
                let recovered = cx.update(|cx| cx.open_window(WindowOptions::default(), |window, cx| cx.new(|cx| Editor::new(recovery_launch, window, cx))).unwrap()).unwrap();
                cx.background_executor().timer(Duration::from_secs(2)).await;
                recovered.update(cx, |view, _, cx| {
                    assert_eq!(view.terminals.active().unwrap().read(cx).working_directory(), a);
                    assert!(view.error.as_ref().is_some_and(|error| error.contains("directory is missing")));
                    cx.global_mut::<crate::preferences::AppPreferences>().0.remember_terminal_layouts = false;
                }).unwrap();
                cx.background_executor().spawn(async { zvim::terminal_workspace::flush(); }).await;
                let before_disable = recovered.read_with(cx, |view, _| zvim::terminal_workspace::load_from(&view.workspace_directory, &a).unwrap()).unwrap();
                recovered.update(cx, |view, _, cx| {
                    view.terminal_fraction = 0.25;
                    view.save_terminal_layouts(cx);
                }).unwrap();
                cx.background_executor().spawn(async { zvim::terminal_workspace::flush(); }).await;
                recovered.update(cx, |view, window, cx| {
                    assert_eq!(zvim::terminal_workspace::load_from(&view.workspace_directory, &a).unwrap(), before_disable, "disabling persistence must leave saved layouts unchanged");
                    view.finish_close(CloseTarget::Window, window, cx);
                }).unwrap();
                eprintln!("PASS: real Neovim project hook, live A/B/A shells, hidden close checks, deferred switches, persisted nested layout/cwd, fresh-shell relaunch, background exit cleanup, missing-directory recovery and disabled persistence");
                cx.update(|cx| cx.quit()).unwrap();
            }).detach();
        });
    }
}

fn main() {
    terminal::configure_resources();
    ui::probe();
}
