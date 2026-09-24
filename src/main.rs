mod preferences;
mod terminal;
mod ui;
use gpui::*;
use zvim::session::Launch;
fn main() {
    terminal::configure_resources();
    zvim::startup::mark("main");
    let cli = match zvim::cli::parse(
        std::env::args_os().skip(1),
        &std::env::current_dir().expect("working directory unavailable"),
    ) {
        Ok(cli) => cli,
        Err(error) => {
            eprintln!("zvim: {error}\nRun zvim --help for usage.");
            std::process::exit(2);
        }
    };
    match cli.mode {
        zvim::cli::Mode::Help | zvim::cli::Mode::Version => {
            use std::io::Write;
            let help = cli.mode == zvim::cli::Mode::Help;
            if help {
                print!("{}", zvim::cli::HELP);
            } else {
                println!("Zvim {}", env!("CARGO_PKG_VERSION"));
            }
            let _ = std::io::stdout().flush();
            let status = zvim::session::bundled_neovim().and_then(|exe| {
                std::process::Command::new(exe)
                    .arg(if help { "--help" } else { "--version" })
                    .status()
                    .map_err(Into::into)
            });
            match status {
                Ok(s) => std::process::exit(s.code().unwrap_or(1)),
                Err(e) => {
                    eprintln!("{e:#}");
                    std::process::exit(1);
                }
            }
        }
        zvim::cli::Mode::Detached => {
            if let Err(error) = detach(&cli.child_args) {
                eprintln!("zvim: {error:#}");
                std::process::exit(1);
            }
            return;
        }
        zvim::cli::Mode::Gui => {}
    }
    let launch = cli.launch;
    zvim::startup::mark("application_create_begin");
    let app = Application::new();
    zvim::startup::mark("application_create_end");
    let (urls_tx, urls_rx) = async_channel::unbounded();
    app.on_open_urls(move |urls| {
        let _ = urls_tx.try_send(urls);
    });
    app.run(move |cx| {
        zvim::startup::mark("application_run");
        cx.set_global(preferences::AppPreferences(
            zvim::settings::Preferences::load().unwrap_or_default(),
        ));
        cx.spawn(async move |cx| {
            while let Ok(urls) = urls_rx.recv().await {
                let paths = urls
                    .into_iter()
                    .filter_map(|s| url::Url::parse(&s).ok()?.to_file_path().ok())
                    .collect::<Vec<_>>();
                if !paths.is_empty() {
                    let _ = cx.update(|cx| ui::open_paths(paths, cx));
                }
            }
        })
        .detach();
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.on_action(|_: &ui::NewWindow, cx| ui::open_window(Launch::default(), cx));
        cx.on_action(|_: &ui::OpenSettings, cx| cx.defer(preferences::open));
        cx.bind_keys([KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-,"
            } else {
                "ctrl-shift-,"
            },
            ui::OpenSettings,
            None,
        )]);
        cx.on_action(|_: &ui::Quit, cx| cx.defer(ui::request_close_all));
        cx.bind_keys([
            KeyBinding::new("ctrl-`", ui::ToggleTerminal, Some("ZvimWindow")),
            KeyBinding::new("cmd-w", ui::Close, Some("ZvimWindow")),
            KeyBinding::new("cmd-q", ui::Quit, None),
            KeyBinding::new("cmd-n", ui::NewWindow, None),
        ]);
        cx.set_menus(vec![
            Menu {
                name: "Zvim".into(),
                items: vec![
                    MenuItem::action("Settings…", ui::OpenSettings),
                    MenuItem::action("New Window", ui::NewWindow),
                    MenuItem::action("Quit Zvim", ui::Quit),
                ],
            },
            Menu {
                name: "File".into(),
                items: vec![
                    MenuItem::action("Open…", ui::Open),
                    MenuItem::action("Close Window", ui::Close),
                ],
            },
            Menu {
                name: "Terminal".into(),
                items: vec![
                    MenuItem::action("Show / Hide Terminal", ui::ToggleTerminal),
                    MenuItem::action("Close Terminal…", ui::CloseTerminal),
                ],
            },
            Menu {
                name: "Edit".into(),
                items: vec![MenuItem::action("Paste", ui::Paste)],
            },
        ]);
        ui::open_window(launch, cx);
    });
}

/// Launch a separate GUI process without tying its lifetime to the terminal.
fn detach(args: &[std::ffi::OsString]) -> anyhow::Result<()> {
    use std::process::{Command, Stdio};
    zvim::session::bundled_neovim()?;
    let dir = zvim::settings::data_dir();
    std::fs::create_dir_all(&dir)?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("launcher.log"))?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--zvim-gui")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn()?;
    Ok(())
}
