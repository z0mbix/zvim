//! Exercise the same embedded UI protocol without opening a GPUI window.
use rmpv::Value;
use std::time::{Duration, Instant};
use zvim::session::{Event, Launch, Session};
fn main() -> anyhow::Result<()> {
    let clean = !std::env::args().any(|a| a == "--config");
    let launch = Launch {
        clean,
        files: vec![std::env::current_dir()?.join("README.md")],
        ..Default::default()
    };
    let started = Instant::now();
    let s = Session::spawn(&launch)?;
    s.initialize(100, 35)?;
    std::thread::sleep(Duration::from_secs(2));
    if std::env::args().any(|a| a == "--appearance") {
        for (label, appearance) in [
            ("initial", None),
            ("light", Some(Some(false))),
            ("dark", Some(Some(true))),
            ("disabled", Some(None)),
        ] {
            if let Some(value) = appearance {
                s.set_appearance(value);
            }
            let value = s.rpc.request("nvim_exec_lua", vec!["local n=package.loaded.nvconfig; return {background=vim.o.background, theme=n and n.base46.theme or '', colors_name=vim.g.colors_name or '', normal=vim.api.nvim_get_hl(0,{name='Normal',link=false})}".into(), Value::Array(vec![])])?;
            println!("{label}: {value}");
        }
        s.rpc.send("nvim_command", vec!["qall!".into()]);
        return Ok(());
    }
    s.input("<Esc><CR><Esc>");
    std::thread::sleep(Duration::from_millis(200));
    for ch in ":set guifont?".chars() {
        s.input(ch.to_string());
        std::thread::sleep(Duration::from_millis(30));
    }
    s.input("<CR>");
    std::thread::sleep(Duration::from_millis(500));
    while let Ok(event) = s.events.try_recv() {
        match event {
            Event::ClipboardPaste(id) => s.rpc.reply(
                id,
                Value::Array(vec![Value::Array(vec!["".into()]), "v".into()]),
            ),
            Event::Error(e) => eprintln!("error: {e}"),
            _ => {}
        }
    }
    if let Some(g) = s.take_frame() {
        for row in g.cells.chunks(g.width) {
            println!(
                "{}",
                row.iter()
                    .map(|c| c.text.as_str())
                    .collect::<String>()
                    .trim_end()
            );
        }
    }
    s.input("<CR><Esc>");
    std::thread::sleep(Duration::from_millis(200));
    if let Ok(messages) = s.rpc.request(
        "nvim_exec2",
        vec![
            "messages".into(),
            Value::Map(vec![("output".into(), true.into())]),
        ],
    ) {
        println!("MESSAGES: {messages}");
    }
    println!(
        "Diagnostic completed in {:?}. Config dir: {}",
        started.elapsed(),
        zvim::settings::data_dir().display()
    );
    s.rpc.send("nvim_command", vec!["qall!".into()]);
    Ok(())
}
