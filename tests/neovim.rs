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
            eval(&s, "getcwd()"),
            dir.canonicalize().unwrap().to_str().unwrap().into()
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
        eval(&s, "g:initial_cwd"),
        dir.canonicalize().unwrap().to_str().unwrap().into()
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
