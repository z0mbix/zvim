use rmpv::Value;
use std::time::{Duration, Instant};
use zvim::session::{Launch, Session};
fn main() {
    for slow in [false, true] {
        for coalesce in [false, true] {
            let state = tempfile::tempdir().unwrap();
            let session = Session::spawn(&Launch {
                clean: true,
                state_directory: Some(state.path().into()),
                ..Default::default()
            })
            .unwrap();
            session.initialize(80, 24).unwrap();
            session.rpc.request("nvim_exec_lua",vec![format!("vim.g.resize_count=0; vim.api.nvim_create_autocmd('VimResized',{{callback=function() vim.g.resize_count=vim.g.resize_count+1; {} end}})",if slow {"vim.uv.sleep(25)"} else {""}).into(),Value::Array(vec![])]).unwrap();
            let start = Instant::now();
            for index in 0..100 {
                let width = 100 + index;
                if coalesce {
                    session.resize(width, 40);
                } else {
                    session
                        .rpc
                        .send("nvim_ui_try_resize", vec![(width as u64).into(), 40.into()]);
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            let submit_ms = start.elapsed().as_secs_f64() * 1000.;
            loop {
                let size = session
                    .rpc
                    .request("nvim_eval", vec!["&columns".into()])
                    .unwrap();
                if size == 199.into() {
                    break;
                }
                assert!(start.elapsed() < Duration::from_secs(10));
                std::thread::sleep(Duration::from_millis(5));
            }
            let settled_ms = start.elapsed().as_secs_f64() * 1000.;
            let calls = session
                .rpc
                .request("nvim_eval", vec!["g:resize_count".into()])
                .unwrap();
            println!(
                "slow={slow} coalesce={coalesce} submitted_ms={submit_ms:.1} settled_ms={settled_ms:.1} callbacks={calls} final_width=199"
            );
        }
    }
}
