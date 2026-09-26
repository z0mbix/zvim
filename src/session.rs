use crate::grid::Grid;
use anyhow::{Context, Result, bail};
use rmpv::Value;
use std::{
    collections::HashMap,
    io::{BufReader, BufWriter, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

pub enum Event {
    Terminal(PathBuf),
    NewTerminal(PathBuf),
    TerminalTheme(crate::terminal_theme::TerminalTheme),
    Frame,
    CloseReturned,
    ClipboardCopy(String),
    ClipboardPaste(u64),
    Error(String),
    Exited(bool),
}
type Response = std::result::Result<Value, String>;
#[derive(Clone)]
pub struct Rpc {
    outbound: mpsc::Sender<Value>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Response>>>>,
    next: Arc<AtomicU64>,
}
impl Rpc {
    fn start_request(
        &self,
        method: &str,
        params: Vec<Value>,
    ) -> Result<(u64, mpsc::Receiver<Response>)> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, tx);
        if self
            .outbound
            .send(Value::Array(vec![
                0.into(),
                id.into(),
                method.into(),
                Value::Array(params),
            ]))
            .is_err()
        {
            self.pending.lock().unwrap().remove(&id);
            bail!("Neovim writer stopped")
        }
        Ok((id, rx))
    }
    /// For worker threads and tests only; never block the GPUI thread on RPC.
    pub fn request(&self, method: &str, params: Vec<Value>) -> Result<Value> {
        let (id, rx) = self.start_request(method, params)?;
        let response = rx.recv_timeout(Duration::from_secs(15));
        self.pending.lock().unwrap().remove(&id);
        response
            .context("Neovim request timed out")?
            .map_err(anyhow::Error::msg)
    }
    pub fn send(&self, method: &str, params: Vec<Value>) {
        let _ = self.outbound.send(Value::Array(vec![
            2.into(),
            method.into(),
            Value::Array(params),
        ]));
    }
    pub fn reply(&self, id: u64, value: Value) {
        let _ = self
            .outbound
            .send(Value::Array(vec![1.into(), id.into(), Value::Nil, value]));
    }
}
pub struct Session {
    pub rpc: Rpc,
    pub events: async_channel::Receiver<Event>,
    frame: Arc<Mutex<Option<Grid>>>,
    input: mpsc::Sender<String>,
    resize: crate::latest_request::LatestRequest<(usize, usize)>,
    child: Arc<Mutex<Child>>,
}
#[derive(Clone, Default)]
pub struct Launch {
    pub clean: bool,
    pub files: Vec<PathBuf>,
    pub nvim_args: Vec<std::ffi::OsString>,
    pub working_directory: Option<PathBuf>,
    /// Optional isolated state root for tests and diagnostics.
    pub state_directory: Option<PathBuf>,
    pub environment: Option<Vec<(std::ffi::OsString, std::ffi::OsString)>>,
}
impl Launch {
    fn env(&self, name: &str) -> Option<std::ffi::OsString> {
        match &self.environment {
            Some(values) => values
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone()),
            None => std::env::var_os(name),
        }
    }
}
pub fn bundled_neovim() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().context("no executable directory")?;
    let bin = "bin/nvim";
    let target = if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "macos-arm64"
        } else {
            "macos-x86_64"
        }
    } else {
        "linux-x86_64"
    };
    let candidates = [
        dir.join("../Resources/neovim").join(bin),
        dir.join("neovim").join(bin),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("vendor")
            .join(target)
            .join("neovim")
            .join(bin),
    ];
    candidates.into_iter().find(|p| p.is_file()).context(
        "Bundled Neovim is missing. Run python3 scripts/bundle-neovim.py, or reinstall Zvim.",
    )
}
/// Desktop launches often omit package-manager paths. Preserve existing entries and append common locations.
fn editor_path(launch: &Launch) -> Result<std::ffi::OsString> {
    #[allow(unused_mut)]
    let mut paths =
        std::env::split_paths(&launch.env("PATH").unwrap_or_default()).collect::<Vec<_>>();
    #[cfg(unix)]
    {
        let mut extra = vec![
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
        ];
        if let Some(home) = launch.env("HOME") {
            extra.push(PathBuf::from(home).join(".local/bin"));
        }
        for p in extra {
            if !paths.contains(&p) {
                paths.push(p);
            }
        }
    }
    std::env::join_paths(paths).context("invalid PATH")
}
impl Session {
    pub fn spawn(launch: &Launch) -> Result<Self> {
        let exe = bundled_neovim()?;
        let root = exe.parent().unwrap().parent().unwrap();
        let mut cmd = Command::new(&exe);
        if let Some(environment) = &launch.environment {
            cmd.env_clear().envs(environment.iter().cloned());
        }
        cmd.arg("--embed")
            .arg("--cmd")
            .arg("let g:zvim = v:true")
            .arg("--cmd")
            .arg("set title");
        if launch.clean {
            cmd.arg("--clean");
        }
        cmd.args(&launch.nvim_args);
        if !launch.files.is_empty() {
            cmd.arg("--").args(&launch.files);
        }
        cmd.env("VIMRUNTIME", root.join("share/nvim/runtime"))
            .env("PATH", editor_path(launch)?)
            .env("ZVIM", "1");
        if let Some(state) = &launch.state_directory {
            for (var, child) in [
                ("XDG_STATE_HOME", "state"),
                ("XDG_DATA_HOME", "data"),
                ("XDG_CACHE_HOME", "cache"),
            ] {
                let path = state.join(child);
                std::fs::create_dir_all(&path)?;
                cmd.env(var, path);
            }
        }
        // Launch Services may use / as cwd; do not make plugin file pickers scan the filesystem root.
        if let Some(directory) = &launch.working_directory {
            cmd.current_dir(directory);
        } else if std::env::current_dir()
            .ok()
            .is_some_and(|p| p.parent().is_none())
        {
            let directory = launch
                .files
                .first()
                .filter(|p| p.is_absolute())
                .and_then(|p| p.parent())
                .map(|p| p.to_owned())
                .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().to_owned()));
            if let Some(directory) = directory {
                cmd.current_dir(directory);
            }
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd
            .spawn()
            .with_context(|| format!("could not launch {}", exe.display()))?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let child = Arc::new(Mutex::new(child));
        let (tx, rx) = mpsc::channel();
        let (events_tx, events) = async_channel::unbounded();
        let pending = Arc::new(Mutex::new(HashMap::<u64, mpsc::Sender<Response>>::new()));
        let rpc = Rpc {
            outbound: tx,
            pending: pending.clone(),
            next: Arc::new(AtomicU64::new(1)),
        };
        let frame = Arc::new(Mutex::new(None));
        let frame_reader = frame.clone();
        let writer_events = events_tx.clone();
        std::thread::spawn(move || {
            let mut out = BufWriter::new(stdin);
            for value in rx {
                if let Err(e) = rmpv::encode::write_value(&mut out, &value)
                    .map_err(anyhow::Error::from)
                    .and_then(|_| out.flush().map_err(anyhow::Error::from))
                {
                    let _ = writer_events
                        .send_blocking(Event::Error(format!("Neovim input closed: {e}")));
                    break;
                }
            }
        });
        let reader_events = events_tx.clone();
        let reader_rpc = rpc.clone();
        std::thread::spawn(move || {
            let mut input = BufReader::new(stdout);
            let mut grid = Grid::default();
            while let Ok(value) = rmpv::decode::read_value(&mut input) {
                let Some(a) = value.as_array() else { continue };
                match a.first().and_then(Value::as_u64) {
                    Some(1) if a.len() >= 4 => {
                        if let Some(tx) = a[1]
                            .as_u64()
                            .and_then(|id| pending.lock().unwrap().remove(&id))
                        {
                            let _ = tx.send(if a[2].is_nil() {
                                Ok(a[3].clone())
                            } else {
                                Err(a[2].to_string())
                            });
                        }
                    }
                    Some(2) if a.len() >= 3 => {
                        let args = a[2].as_array().cloned().unwrap_or_default();
                        match a[1].as_str() {
                            Some("redraw") => {
                                if args.iter().any(|event| {
                                    matches!(
                                        event
                                            .as_array()
                                            .and_then(|a| a.first())
                                            .and_then(Value::as_str),
                                        Some("default_colors_set" | "hl_attr_define")
                                    )
                                }) {
                                    reader_rpc.send("nvim_exec_lua", vec!["if _G.__zvim_terminal_theme_refresh then _G.__zvim_terminal_theme_refresh() end".into(), Value::Array(vec![])]);
                                }
                                crate::startup::mark("redraw_decode_begin");
                                let result = grid.redraw(&args);
                                crate::startup::mark("redraw_decode_end");
                                match result {
                                    Ok(frames) => {
                                        if let Some(last) = frames.into_iter().last() {
                                            let wake = frame_reader
                                                .lock()
                                                .unwrap()
                                                .replace(last)
                                                .is_none();
                                            if wake {
                                                let _ = reader_events.send_blocking(Event::Frame);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let _ = reader_events.send_blocking(Event::Error(format!(
                                            "UI protocol: {e}"
                                        )));
                                    }
                                }
                            }
                            Some("nvim_error_event") => {
                                let message = args
                                    .get(1)
                                    .and_then(Value::as_str)
                                    .unwrap_or("Neovim API error");
                                let _ = reader_events.send_blocking(Event::Error(message.into()));
                            }
                            Some("zvim_clipboard_copy") => {
                                let lines = args
                                    .first()
                                    .and_then(Value::as_array)
                                    .cloned()
                                    .unwrap_or_default();
                                let mut text = lines
                                    .iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                if args.get(1).and_then(Value::as_str) == Some("V")
                                    && !text.ends_with('\n')
                                {
                                    text.push('\n');
                                }
                                let _ = reader_events.send_blocking(Event::ClipboardCopy(text));
                            }
                            Some("zvim_terminal_theme") => {
                                if let Some(theme) = args
                                    .first()
                                    .and_then(crate::terminal_theme::TerminalTheme::from_value)
                                {
                                    let _ =
                                        reader_events.send_blocking(Event::TerminalTheme(theme));
                                }
                            }
                            Some("zvim_close_returned") => {
                                let _ = reader_events.send_blocking(Event::CloseReturned);
                            }
                            Some(name @ ("zvim_terminal" | "zvim_new_terminal")) => {
                                if let Some(cwd) = args.first().and_then(Value::as_str) {
                                    let _ = reader_events.send_blocking(
                                        if name == "zvim_new_terminal" {
                                            Event::NewTerminal(PathBuf::from(cwd))
                                        } else {
                                            Event::Terminal(PathBuf::from(cwd))
                                        },
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                    Some(0) if a.len() >= 4 => {
                        let id = a[1].as_u64().unwrap_or(0);
                        if a[2].as_str() == Some("zvim_clipboard_paste") {
                            let _ = reader_events.send_blocking(Event::ClipboardPaste(id));
                        } else {
                            reader_rpc.reply(id, Value::Nil);
                        }
                    }
                    _ => {}
                }
            }
            for (_, tx) in pending.lock().unwrap().drain() {
                let _ = tx.send(Err("Neovim disconnected".into()));
            }
        });
        std::thread::spawn(move || {
            use std::io::BufRead;
            let dir = crate::settings::data_dir();
            let _ = std::fs::create_dir_all(&dir);
            let mut log = std::fs::File::create(dir.join("neovim.log")).ok();
            for line in BufReader::new(stderr)
                .lines()
                .map_while(std::result::Result::ok)
            {
                eprintln!("neovim: {line}");
                if let Some(f) = log.as_mut() {
                    let _ = writeln!(f, "{line}");
                }
            }
        });
        let watch = child.clone();
        let exit_events = events_tx.clone();
        std::thread::spawn(move || {
            loop {
                match watch.lock().unwrap().try_wait() {
                    Ok(Some(status)) => {
                        let _ = exit_events.send_blocking(Event::Exited(status.success()));
                        break;
                    }
                    Err(e) => {
                        let _ = exit_events.send_blocking(Event::Error(e.to_string()));
                        break;
                    }
                    _ => {}
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let (input_tx, input_rx) = mpsc::channel::<String>();
        let input_rpc = rpc.clone();
        let resize_events = events_tx.clone();
        std::thread::spawn(move || {
            for keys in input_rx {
                let mut remaining = keys.as_str();
                while !remaining.is_empty() {
                    crate::startup::mark("input_rpc_begin");
                    let response = input_rpc.request("nvim_input", vec![remaining.into()]);
                    crate::startup::mark("input_rpc_end");
                    match response {
                        Ok(v) => {
                            let n = v.as_u64().unwrap_or(0) as usize;
                            if n > remaining.len() || !remaining.is_char_boundary(n) {
                                break;
                            }
                            remaining = &remaining[n..];
                            if n == 0 {
                                std::thread::sleep(Duration::from_millis(1));
                            }
                        }
                        Err(e) => {
                            let _ = events_tx.send_blocking(Event::Error(e.to_string()));
                            break;
                        }
                    }
                }
            }
        });
        let resize_rpc = rpc.clone();
        let resize =
            crate::latest_request::LatestRequest::new(
                move |(w, h): (usize, usize)| match resize_rpc.request(
                    "nvim_ui_try_resize",
                    vec![(w as u64).into(), (h as u64).into()],
                ) {
                    Ok(_) => true,
                    Err(error) => {
                        let _ = resize_events
                            .send_blocking(Event::Error(format!("Neovim resize failed: {error}")));
                        false
                    }
                },
            );
        Ok(Self {
            resize,
            rpc,
            events,
            frame,
            input: input_tx,
            child,
        })
    }
    pub fn new_terminal(&self) {
        self.rpc.send(
            "nvim_exec_lua",
            vec![
                "vim.rpcnotify(vim.g.zvim_channel, 'zvim_new_terminal', vim.fn.getcwd())".into(),
                Value::Array(vec![]),
            ],
        );
    }

    pub fn set_appearance(&self, dark: Option<bool>) {
        self.rpc.send(
            "nvim_exec_lua",
            vec![
                include_str!("appearance.lua").into(),
                Value::Array(vec![
                    dark.map(|d| Value::from(if d { "dark" } else { "light" }))
                        .unwrap_or(Value::Nil),
                ]),
            ],
        );
    }
    pub fn initialize(&self, width: u64, height: u64) -> Result<()> {
        initialize(&self.rpc, width, height)
    }
    pub fn take_frame(&self) -> Option<Grid> {
        self.frame.lock().unwrap().take()
    }
    pub fn input(&self, text: impl Into<String>) {
        crate::startup::mark("input_submit");
        let _ = self.input.send(text.into());
    }
    pub fn resize(&self, w: usize, h: usize) {
        self.resize.submit((w, h));
    }
    pub fn paste(&self, text: String) {
        self.rpc
            .send("nvim_paste", vec![text.into(), true.into(), (-1).into()]);
    }
    pub fn close(&self) {
        self.rpc.send("nvim_exec_lua", vec![
            "local ok, err = pcall(vim.cmd, 'confirm qall'); if not ok then vim.api.nvim_err_writeln(tostring(err)) end; vim.rpcnotify(vim.g.zvim_channel, 'zvim_close_returned')".into(),
            Value::Array(vec![]),
        ]);
    }
    pub fn open_files(&self, paths: &[PathBuf]) {
        let files = Value::Array(
            paths
                .iter()
                .map(|p| p.to_string_lossy().to_string().into())
                .collect(),
        );
        self.rpc.send("nvim_exec_lua",vec!["local paths = ...; for _,p in ipairs(paths) do vim.cmd('argadd '..vim.fn.fnameescape(p)) end; if paths[1] then vim.cmd('edit '..vim.fn.fnameescape(paths[1])) end".into(),Value::Array(vec![files])]);
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock()
            && child.try_wait().ok().flatten().is_none()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
pub fn initialize(rpc: &Rpc, width: u64, height: u64) -> Result<()> {
    crate::startup::mark("rpc_initialize_begin");
    let api = rpc.request("nvim_get_api_info", vec![])?;
    let channel = api
        .as_array()
        .and_then(|a| a.first())
        .and_then(Value::as_u64)
        .context("missing channel id")?;
    rpc.request(
        "nvim_set_client_info",
        vec![
            "Zvim".into(),
            Value::Map(vec![("major".into(), 0.into()), ("minor".into(), 1.into())]),
            "ui".into(),
            Value::Map(vec![]),
            Value::Map(vec![]),
        ],
    )?;
    // Set up provider before UI attach releases Neovim's startup/configuration barrier.
    rpc.request(
        "nvim_exec_lua",
        vec![
            include_str!("clipboard.lua").into(),
            Value::Array(vec![channel.into()]),
        ],
    )?;
    rpc.request(
        "nvim_exec_lua",
        vec![
            include_str!("terminal_theme.lua").into(),
            Value::Array(vec![channel.into()]),
        ],
    )?;
    crate::startup::mark("ui_attach_begin");
    rpc.request("nvim_exec_lua", vec![
        "local channel = ...; vim.g.zvim_channel = channel; vim.api.nvim_create_user_command('ZvimTerminal', function() vim.rpcnotify(channel, 'zvim_terminal', vim.fn.getcwd()) end, {desc='Toggle Zvim native terminal'})".into(),
        Value::Array(vec![channel.into()]),
    ])?;
    rpc.request(
        "nvim_ui_attach",
        vec![
            width.into(),
            height.into(),
            Value::Map(vec![
                ("rgb".into(), true.into()),
                ("ext_linegrid".into(), true.into()),
            ]),
        ],
    )?;
    crate::startup::mark("ui_attach_end");
    Ok(())
}
