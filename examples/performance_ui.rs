//! Opt-in real-window benchmark. Run through scripts/benchmark-ui.py.
#![allow(dead_code)]
#[path = "../src/about.rs"]
mod about;
#[path = "../src/preferences.rs"]
mod preferences;
#[path = "../src/terminal.rs"]
mod terminal;
#[path = "../src/terminal_search.rs"]
mod terminal_search;
#[path = "../src/ui.rs"]
mod ui;
use gpui::*;
use std::time::Duration;

fn main() {
    terminal::configure_resources();
    let state = tempfile::tempdir().unwrap();
    let script = state.path().join("workload.lua");
    std::fs::write(&script, r#"
vim.o.guifont = 'Menlo:h13'
vim.o.swapfile = false
vim.o.laststatus = 0
local lines = {}
for i=1,2000 do
  lines[i] = string.format('local value_%04d = "Sample text 日本語 👩‍💻 é" -- scroll and redraw %s', i, string.rep('x', i % 70))
end
vim.api.nvim_buf_set_lines(0,0,-1,false,lines)
vim.bo.filetype = 'lua'
vim.cmd('syntax on')
local step = 0
local timer = vim.uv.new_timer()
timer:start(1500, 16, vim.schedule_wrap(function()
  step = step + 1
  local row = 1 + (step * 3) % 1900
  vim.api.nvim_win_set_cursor(0, {row, step % 20})
  vim.api.nvim_buf_set_text(0,row-1,0,row-1,0,{' '})
  if step % 100 == 0 then vim.cmd('redraw!') end
end))
"#).unwrap();
    Application::new().run(move |cx| {
        cx.set_global(preferences::AppPreferences(
            zvim::settings::Preferences::default(),
        ));
        let launch = zvim::session::Launch {
            state_directory: Some(state.path().to_owned()),
            nvim_args: vec![
                "-u".into(),
                "NONE".into(),
                "-i".into(),
                "NONE".into(),
                "-S".into(),
                script.into(),
            ],
            ..Default::default()
        };
        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(100.), px(100.)),
                        size(px(1200.), px(800.)),
                    ))),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| ui::Editor::new(launch, window, cx)),
            )
            .unwrap();
        cx.activate(true);
        cx.spawn(async move |cx| {
            // Keep diagnostics and Neovim state alive until the application exits.
            let _state = state;
            cx.background_executor().timer(Duration::from_secs(2)).await;
            for phase in ["editor", "hidden_tabs", "splits", "resize"] {
                if phase == "hidden_tabs" {
                    for _ in 0..8 {
                        handle
                            .update(cx, |_, w, cx| {
                                w.dispatch_action(Box::new(ui::NewTerminal), cx)
                            })
                            .unwrap();
                        cx.background_executor()
                            .timer(Duration::from_millis(200))
                            .await;
                    }
                    handle
                        .update(cx, |_, w, cx| {
                            w.dispatch_action(Box::new(ui::ToggleTerminal), cx)
                        })
                        .unwrap();
                } else if phase == "splits" {
                    handle
                        .update(cx, |_, w, cx| {
                            w.dispatch_action(Box::new(ui::ToggleTerminal), cx)
                        })
                        .unwrap();
                    for right in [true, false] {
                        handle
                            .update(cx, |_, w, cx| {
                                let action: Box<dyn Action> = if right {
                                    Box::new(ui::SplitTerminalRight)
                                } else {
                                    Box::new(ui::SplitTerminalDown)
                                };
                                w.dispatch_action(action, cx);
                            })
                            .unwrap();
                        cx.background_executor()
                            .timer(Duration::from_millis(300))
                            .await;
                    }
                }
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                eprintln!("benchmark_phase={phase}");
                for step in 0..180 {
                    handle
                        .update(cx, |_, w, _| {
                            if phase == "resize" {
                                w.resize(size(
                                    px(1100. + (step % 40) as f32 * 4.),
                                    px(720. + (step % 30) as f32 * 3.),
                                ));
                            }
                            w.refresh();
                        })
                        .unwrap();
                    cx.background_executor()
                        .timer(Duration::from_millis(16))
                        .await;
                }
                eprintln!("benchmark_phase=transition");
            }
            cx.update(|cx| cx.quit()).unwrap();
        })
        .detach();
    });
}
