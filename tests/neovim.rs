use rmpv::Value;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use zvim::session::{Event, Launch, Session};
fn eval(s: &Session, expr: &str) -> Value {
    s.rpc.request("nvim_eval", vec![expr.into()]).unwrap()
}
fn command(s: &Session, cmd: &str) {
    s.rpc.request("nvim_command", vec![cmd.into()]).unwrap();
}
fn wait(s: &Session, predicate: impl Fn() -> bool) {
    let start = Instant::now();
    while !predicate() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "condition timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = s;
}
#[test]
fn bundled_editor_roundtrip() {
    let dir = std::env::temp_dir().join(format!("zvim-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("file with ' quotes.txt");
    std::fs::write(&path, "original\n").unwrap();
    let s = Session::spawn(&Launch {
        clean: true,
        files: vec![path.clone()],
        state_directory: Some(dir.join("isolated")),
        ..Default::default()
    })
    .unwrap();
    s.initialize(80, 24).unwrap();
    assert_eq!(eval(&s, "g:zvim"), true.into());
    assert_eq!(eval(&s, "has('nvim-0.12')"), 1.into());
    s.input("gg0ihello <LT>world> <Esc>");
    wait(&s, || {
        eval(&s, "getline(1)")
            .as_str()
            .is_some_and(|s| s.starts_with("hello <world>"))
    });
    command(&s, "write");
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("hello <world>")
    );
    command(&s, "vsplit");
    assert_eq!(eval(&s, "winnr('$')"), 2.into());
    s.rpc.request("nvim_exec_lua",vec!["local b=vim.api.nvim_create_buf(false,true); vim.api.nvim_buf_set_lines(b,0,-1,false,{'界 é 😀'}); return vim.api.nvim_open_win(b,false,{relative='editor',row=2,col=2,width=20,height=3})".into(),Value::Array(vec![])]).unwrap();
    s.resize(95, 31);
    wait(&s, || {
        eval(&s, "&columns") == 95.into() && eval(&s, "&lines") == 31.into()
    });
    let second = dir.join("second.txt");
    std::fs::write(&second, "two\n").unwrap();
    s.open_files(std::slice::from_ref(&second));
    wait(&s, || eval(&s, "expand('%:p')").as_str() == second.to_str());
    s.paste("α\nbeta".into());
    wait(&s, || eval(&s, "&modified") == 1.into());
    s.close();
    // A confirmation must keep the process alive and accept input while the command is pending.
    std::thread::sleep(Duration::from_millis(100));
    s.input("c");
    wait(&s, || eval(&s, "mode()") == "n".into());
    assert_eq!(eval(&s, "&modified"), 1.into());
    s.close();
    std::thread::sleep(Duration::from_millis(100));
    s.input("n");
    let start = Instant::now();
    let mut exited = false;
    while start.elapsed() < Duration::from_secs(5) {
        match s.events.try_recv() {
            Ok(Event::Exited(ok)) => {
                assert!(ok);
                exited = true;
                break;
            }
            Ok(Event::ClipboardPaste(id)) => s.rpc.reply(
                id,
                Value::Array(vec![Value::Array(vec!["".into()]), "v".into()]),
            ),
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        exited,
        "confirm discard should exit: {:?}",
        s.take_frame()
            .map(|g| g.cells.iter().map(|c| c.text.clone()).collect::<String>())
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn clipboard_rpc_is_bidirectional() {
    let s = Session::spawn(&Launch {
        clean: true,
        files: Vec::<PathBuf>::new(),
        state_directory: Some(
            std::env::temp_dir().join(format!("zvim-clipboard-{}", std::process::id())),
        ),
        ..Default::default()
    })
    .unwrap();
    s.initialize(60, 20).unwrap();
    command(&s, "call setline(1, 'clipboard sample')");
    s.input("gg\"+yy");
    let start = Instant::now();
    let mut copied = false;
    while start.elapsed() < Duration::from_secs(5) {
        if let Ok(Event::ClipboardCopy(text)) = s.events.try_recv() {
            assert_eq!(text, "clipboard sample\n");
            copied = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(copied);
    s.input("\"+p");
    let start = Instant::now();
    let mut pasted = false;
    while start.elapsed() < Duration::from_secs(5) {
        if let Ok(Event::ClipboardPaste(id)) = s.events.try_recv() {
            s.rpc.reply(
                id,
                Value::Array(vec![Value::Array(vec!["from system".into()]), "V".into()]),
            );
            pasted = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(pasted);
    wait(&s, || eval(&s, "getline(2)") == "from system".into());
    s.rpc.send("nvim_command", vec!["qall!".into()]);
}

#[test]
fn cli_layout_flags_reach_neovim() {
    let dir = std::env::temp_dir().join(format!("zvim-cli-layout-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("one.txt"), "one\n").unwrap();
    std::fs::write(dir.join("two.txt"), "two\n").unwrap();
    for (flag, expression, expected) in [
        ("-O", "winlayout()[0]", "row".into()),
        ("-o", "winlayout()[0]", "col".into()),
        ("-p", "tabpagenr('$')", 2.into()),
        ("-R", "&readonly", 1.into()),
        ("-d", "&diff", 1.into()),
    ] {
        let mut cli = zvim::cli::parse(
            ["--clean", ".", flag, "one.txt", "two.txt"].map(Into::into),
            &dir,
        )
        .unwrap();
        cli.launch.state_directory = Some(dir.join("state"));
        let s = Session::spawn(&cli.launch).unwrap();
        s.initialize(80, 24).unwrap();
        assert_eq!(eval(&s, expression), expected, "{flag}");
        assert_eq!(eval(&s, "argc()"), 2.into());
        assert_eq!(
            std::path::Path::new(eval(&s, "getcwd()").as_str().unwrap())
                .canonicalize()
                .unwrap(),
            dir.canonicalize().unwrap()
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cli_project_directory_precedes_configuration_and_commands() {
    let dir = std::env::temp_dir().join(format!("zvim cli startup {}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("init.lua"), "vim.g.initial_cwd = vim.fn.getcwd(); vim.g.initial_zvim = vim.g.zvim; vim.g.sequence = vim.g.sequence .. 'config'\n").unwrap();
    std::fs::write(dir.join("file with spaces.txt"), "one\ntwo\nthree\nfour\n").unwrap();
    let mut cli = zvim::cli::parse(
        [
            "--cmd",
            "let g:sequence = 'before-'",
            "-u",
            "init.lua",
            ".",
            "file with spaces.txt",
            "-c",
            "let g:sequence .= '-after'",
            "+3",
        ]
        .map(Into::into),
        &dir,
    )
    .unwrap();
    cli.launch.state_directory = Some(dir.join("state"));
    let s = Session::spawn(&cli.launch).unwrap();
    s.initialize(80, 24).unwrap();
    assert_eq!(
        std::path::Path::new(eval(&s, "g:initial_cwd").as_str().unwrap())
            .canonicalize()
            .unwrap(),
        dir.canonicalize().unwrap()
    );
    assert_eq!(eval(&s, "g:initial_zvim"), true.into());
    assert_eq!(eval(&s, "g:sequence"), "before-config-after".into());
    assert_eq!(eval(&s, "line('.')"), 3.into());
    assert_eq!(eval(&s, "expand('%:t')"), "file with spaces.txt".into());
    drop(s);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn appearance_sync_restores_configuration_and_preserves_manual_changes() {
    let s = Session::spawn(&Launch {
        clean: true,
        state_directory: Some(
            std::env::temp_dir().join(format!("zvim-appearance-{}", std::process::id())),
        ),
        ..Default::default()
    })
    .unwrap();
    s.initialize(80, 24).unwrap();
    command(&s, "set background=light");
    s.set_appearance(Some(true));
    wait(&s, || eval(&s, "&background") == "dark".into());
    s.set_appearance(Some(false));
    wait(&s, || eval(&s, "&background") == "light".into());
    s.set_appearance(Some(true));
    wait(&s, || eval(&s, "&background") == "dark".into());
    s.set_appearance(None);
    wait(&s, || eval(&s, "&background") == "light".into());
    command(&s, "set background=dark");
    s.set_appearance(Some(true));
    // An explicit user change after enabling sync should survive disabling it.
    command(&s, "set background=light");
    s.set_appearance(None);
    assert_eq!(eval(&s, "&background"), "light".into());
}

#[test]
fn appearance_sync_preserves_base46_palette() {
    let s = Session::spawn(&Launch {
        clean: true,
        state_directory: Some(
            std::env::temp_dir().join(format!("zvim-base46-{}", std::process::id())),
        ),
        ..Default::default()
    })
    .unwrap();
    s.initialize(80, 24).unwrap();
    s.rpc
        .request(
            "nvim_exec_lua",
            vec![
                r#"
        vim.o.background = 'dark'
        vim.g.colors_name = nil
        vim.g.base46_cache = 'fixture'
        package.loaded.nvconfig = { base46 = { theme = 'rose-pine-moon' } }
        local function reload()
          vim.o.background = 'dark'
          vim.api.nvim_set_hl(0, 'Normal', { fg = '#e0def4', bg = '#232136' })
        end
        package.loaded.base46 = { load_all_highlights = reload }
        reload()
    "#
                .into(),
                Value::Array(vec![]),
            ],
        )
        .unwrap();
    let snapshot = "[&background, luaeval('package.loaded.nvconfig.base46.theme'), nvim_get_hl(0, {'name':'Normal', 'link':v:false})]";
    let original = eval(&s, snapshot);
    for appearance in [Some(false), Some(true), None] {
        s.set_appearance(appearance);
        assert_eq!(eval(&s, snapshot), original);
    }
    // Recover the reset highlights left by the former background-only implementation.
    s.rpc.request("nvim_exec_lua", vec!["_G.__zvim_appearance = {previous='dark', applied='light'}; vim.o.background='light'".into(), Value::Array(vec![])]).unwrap();
    s.set_appearance(None);
    assert_eq!(eval(&s, snapshot), original);
}

#[test]
fn native_terminal_bridge_uses_current_window_directory() {
    let dir = std::env::temp_dir().join(format!("zvim-terminal-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let local = dir.join("project with ' quotes");
    std::fs::create_dir_all(&local).unwrap();
    let s = Session::spawn(&Launch {
        clean: true,
        working_directory: Some(dir.clone()),
        state_directory: Some(dir.join("isolated")),
        ..Default::default()
    })
    .unwrap();
    s.initialize(80, 24).unwrap();
    s.rpc
        .request(
            "nvim_exec_lua",
            vec![
                "vim.cmd.lcd(vim.fn.fnameescape(...))".into(),
                Value::Array(vec![local.to_str().unwrap().into()]),
            ],
        )
        .unwrap();
    command(&s, "ZvimTerminal");
    let start = Instant::now();
    loop {
        if let Ok(Event::Terminal(cwd)) = s.events.try_recv() {
            assert_eq!(cwd.canonicalize().unwrap(), local.canonicalize().unwrap());
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "terminal notification missing"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    // The GUI command must not replace Neovim's own terminal buffers.
    command(&s, "terminal");
    assert_eq!(eval(&s, "&buftype"), "terminal".into());
    drop(s);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn native_terminal_theme_tracks_colours_and_base46_without_restarting_editor() {
    fn theme(
        s: &Session,
        predicate: impl Fn(&zvim::terminal_theme::TerminalTheme) -> bool,
    ) -> zvim::terminal_theme::TerminalTheme {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(Event::TerminalTheme(theme)) = s.events.try_recv()
                && predicate(&theme)
            {
                return theme;
            }
            if Instant::now() >= deadline {
                panic!(
                    "terminal theme timed out: {:?}",
                    s.take_frame().map(|g| g
                        .cells
                        .iter()
                        .map(|c| c.text.as_str())
                        .collect::<String>())
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let s = Session::spawn(&Launch {
        clean: true,
        state_directory: Some(
            std::env::temp_dir().join(format!("zvim-theme-{}", std::process::id())),
        ),
        ..Default::default()
    })
    .unwrap();
    s.initialize(80, 24).unwrap();
    theme(&s, |_| true);
    let pid = eval(&s, "getpid()");
    command(&s, "hi Normal guifg=#abcdef guibg=#123456");
    let direct = theme(&s, |t| t.0[0] == Some(0xabcdef) && t.0[1] == Some(0x123456));
    assert!(direct.config(0, 0).contains("background = #123456"));
    command(
        &s,
        "let g:terminal_color_1 = '#fedcba' | doautocmd ColorScheme",
    );
    theme(&s, |t| t.0[7] == Some(0xfedcba));
    command(&s, "hi Visual guifg=#112233 guibg=#445566 gui=reverse");
    theme(&s, |t| t.0[4] == Some(0x445566) && t.0[5] == Some(0x112233));
    command(&s, "unlet g:terminal_color_1 | doautocmd ColorScheme");
    theme(&s, |t| t.0[7].is_none());
    // Model Base46's direct reload: its terminal globals can still be stale.
    command(
        &s,
        "lua vim.g.colors_name=nil; vim.g.terminal_color_1='#010101'; package.loaded.nvconfig={}; package.loaded.base46={get_theme_tb=function() return {base08='#aabbcc'} end}; vim.api.nvim_exec_autocmds('User',{pattern='NvThemeReload'})",
    );
    theme(&s, |t| t.0[7] == Some(0xaabbcc));
    assert_eq!(eval(&s, "getpid()"), pid);
    s.rpc.send("nvim_command", vec!["qa!".into()]);
}

#[test]
fn cancelling_gui_close_reports_completion_and_keeps_unsaved_editor_alive() {
    let s = Session::spawn(&Launch {
        clean: true,
        state_directory: Some(
            std::env::temp_dir().join(format!("zvim-close-{}", std::process::id())),
        ),
        ..Default::default()
    })
    .unwrap();
    s.initialize(80, 24).unwrap();
    command(&s, "call setline(1, 'keep this unsaved text')");
    let pid = eval(&s, "getpid()");
    s.close();
    // Wait for the actual save prompt before cancelling it.
    wait(&s, || {
        s.take_frame().is_some_and(|grid| {
            grid.cells
                .iter()
                .map(|cell| cell.text.as_str())
                .collect::<String>()
                .contains("ancel")
        })
    });
    s.input("c");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match s.events.try_recv() {
            Ok(Event::CloseReturned) => break,
            Ok(Event::Exited(_)) => panic!("cancelled close exited Neovim"),
            _ => {}
        }
        assert!(Instant::now() < deadline, "close cancellation not reported");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(eval(&s, "getpid()"), pid);
    assert_eq!(eval(&s, "getline(1)"), "keep this unsaved text".into());
    assert_eq!(eval(&s, "&modified"), 1.into());
    s.rpc.send("nvim_command", vec!["qa!".into()]);
}
