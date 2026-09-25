mod about;
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
        cx.on_app_quit(|cx| {
            cx.background_executor().spawn(async {
                zvim::geometry_writer::flush();
            })
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
        cx.on_action(|_: &ui::About, cx| cx.defer(about::open));
        cx.on_action(|_: &ui::Quit, cx| cx.defer(ui::request_close_all));
        preferences::bind_keys(cx);
        cx.observe_global::<preferences::AppPreferences>(preferences::bind_keys)
            .detach();
        cx.set_menus(vec![
            Menu {
                name: "Zvim".into(),
                items: vec![
                    MenuItem::action("About Zvim", ui::About),
                    MenuItem::Separator,
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
                    MenuItem::action("Focus Terminal / Editor", ui::FocusTerminal),
                    MenuItem::action("New Terminal Tab", ui::NewTerminal),
                    MenuItem::action("Split Right", ui::SplitTerminalRight),
                    MenuItem::action("Split Down", ui::SplitTerminalDown),
                    MenuItem::action("Focus Pane Left", ui::TerminalPaneLeft),
                    MenuItem::action("Focus Pane Right", ui::TerminalPaneRight),
                    MenuItem::action("Focus Pane Above", ui::TerminalPaneUp),
                    MenuItem::action("Focus Pane Below", ui::TerminalPaneDown),
                    MenuItem::action("Previous Terminal Tab", ui::PreviousTerminal),
                    MenuItem::action("Next Terminal Tab", ui::NextTerminal),
                    MenuItem::action("Maximise / Restore Terminal", ui::MaximizeTerminal),
                    MenuItem::action("Close Terminal Tab…", ui::CloseTerminal),
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
