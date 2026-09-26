//! Same-user, single-instance launch handoff. No IPC work runs on the UI thread.
use crate::session::Launch;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions, TryLockError},
    io::{Read, Write},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{MetadataExt, OpenOptionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const LIMIT: usize = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Serialize, Deserialize)]
pub struct Message {
    version: u32,
    args: Vec<Vec<u8>>,
    cwd: Vec<u8>,
    environment: Vec<(Vec<u8>, Vec<u8>)>,
    wait: bool,
}
impl Message {
    pub fn new(launch: &Launch, wait: bool) -> Result<Self> {
        Ok(Self {
            version: 1,
            args: launch
                .nvim_args
                .iter()
                .map(|s| s.as_bytes().to_vec())
                .collect(),
            cwd: launch
                .working_directory
                .clone()
                .unwrap_or(std::env::current_dir()?)
                .as_os_str()
                .as_bytes()
                .to_vec(),
            environment: std::env::vars_os()
                .map(|(k, v)| (k.as_bytes().to_vec(), v.as_bytes().to_vec()))
                .collect(),
            wait,
        })
    }
    fn launch(&self) -> Result<Launch> {
        if self.version != 1 {
            bail!("Incompatible running Zvim; quit it and reopen Zvim");
        }
        let cwd = PathBuf::from(std::ffi::OsString::from_vec(self.cwd.clone()));
        if !cwd.is_absolute() || self.cwd.contains(&0) {
            bail!("Invalid launch directory");
        }
        if self.args.iter().any(|arg| arg.contains(&0))
            || self
                .environment
                .iter()
                .any(|(k, v)| k.is_empty() || k.contains(&b'=') || k.contains(&0) || v.contains(&0))
        {
            bail!("Invalid launch arguments or environment");
        }
        Ok(Launch {
            nvim_args: self
                .args
                .iter()
                .cloned()
                .map(std::ffi::OsString::from_vec)
                .collect(),
            working_directory: Some(cwd),
            environment: Some(
                self.environment
                    .iter()
                    .map(|(k, v)| {
                        (
                            std::ffi::OsString::from_vec(k.clone()),
                            std::ffi::OsString::from_vec(v.clone()),
                        )
                    })
                    .collect(),
            ),
            ..Default::default()
        })
    }
}
#[derive(Serialize, Deserialize)]
enum Reply {
    Opened,
    Closed,
    Error(String),
}

pub struct Incoming {
    pub launch: Launch,
    response: mpsc::Sender<Reply>,
}
impl Incoming {
    pub fn opened(self) -> Completion {
        let _ = self.response.send(Reply::Opened);
        Completion(self.response)
    }
    pub fn reject(self, error: String) {
        let _ = self.response.send(Reply::Error(error));
    }
}
/// Held by the requested editor view, not by the whole application.
pub struct Completion(mpsc::Sender<Reply>);
impl Drop for Completion {
    fn drop(&mut self) {
        let _ = self.0.send(Reply::Closed);
    }
}
fn write_message(stream: &mut UnixStream, message: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(message)?;
    if bytes.len() > LIMIT {
        bail!("Launch request exceeds 1 MiB");
    }
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.write_all(&bytes)?;
    Ok(())
}
fn read_message<T: DeserializeOwned>(stream: &mut UnixStream) -> Result<T> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length > LIMIT {
        bail!("Launch request exceeds 1 MiB");
    }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn forward(mut stream: UnixStream, message: &Message) -> Result<()> {
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    write_message(&mut stream, message)?;
    match read_message::<Reply>(&mut stream)? {
        Reply::Opened => {}
        Reply::Error(error) => bail!("{error}"),
        Reply::Closed => bail!("Zvim closed before accepting the window"),
    }
    if message.wait {
        stream.set_read_timeout(None)?;
        match read_message::<Reply>(&mut stream)
            .context("Running Zvim disconnected before the requested window closed")?
        {
            Reply::Closed => {}
            _ => bail!("Unexpected window completion response"),
        }
    }
    Ok(())
}
pub fn connect(dir: &Path) -> Result<UnixStream> {
    let endpoint = std::fs::read(dir.join("instance.endpoint"))?;
    let path = PathBuf::from(std::ffi::OsString::from_vec(endpoint));
    let stream = UnixStream::connect(path)?;
    same_user(&stream)?;
    Ok(stream)
}
fn same_user(stream: &UnixStream) -> Result<()> {
    use std::os::fd::AsRawFd;
    #[cfg(target_os = "macos")]
    let uid = {
        let (mut uid, mut gid) = (0, 0);
        // SAFETY: valid socket and writable output pointers.
        if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        uid
    };
    #[cfg(target_os = "linux")]
    let uid = {
        let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: buffer and length describe a valid ucred output structure.
        if unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut length,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        credentials.uid
    };
    // SAFETY: geteuid has no preconditions.
    if uid != unsafe { libc::geteuid() } {
        bail!("Zvim launch peer belongs to another user");
    }
    Ok(())
}
type Pending = Arc<(Mutex<HashMap<u64, mpsc::Sender<Reply>>>, Condvar)>;
pub enum Role {
    Primary(Server),
    Secondary(UnixStream),
}
pub struct Server {
    pub requests: async_channel::Receiver<Incoming>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    socket: PathBuf,
    pending: Pending,
    _directory: tempfile::TempDir,
    _lock: File,
}
impl Server {
    pub fn acquire(dir: &Path) -> Result<Role> {
        std::fs::create_dir_all(dir)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(dir.join("instance.lock"))?;
        let metadata = lock.metadata()?;
        // SAFETY: geteuid has no preconditions and does not mutate process state.
        if !metadata.is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            bail!("Unsafe Zvim instance lock permissions");
        }
        let deadline = Instant::now() + TIMEOUT;
        loop {
            match lock.try_lock() {
                Ok(()) => break,
                Err(TryLockError::WouldBlock) => {
                    if let Ok(stream) = connect(dir) {
                        return Ok(Role::Secondary(stream));
                    }
                    if Instant::now() >= deadline {
                        bail!("Running Zvim is not responding; quit it and retry");
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(TryLockError::Error(error)) => return Err(error.into()),
            }
        }
        // A short, private path avoids Unix socket path-length limits on macOS.
        let directory = tempfile::Builder::new()
            .prefix("zvim-ipc-")
            .tempdir_in("/tmp")?;
        let socket = directory.path().join("launch.sock");
        let listener = UnixListener::bind(&socket)?;
        let mut endpoint = tempfile::NamedTempFile::new_in(dir)?;
        endpoint.write_all(socket.as_os_str().as_bytes())?;
        endpoint.persist(dir.join("instance.endpoint"))?;
        let (sender, requests) = async_channel::unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let pending: Pending = Default::default();
        let clients = pending.clone();
        let thread = std::thread::Builder::new()
            .name("zvim-launch-listener".into())
            .spawn(move || {
                for (id, connection) in listener.incoming().enumerate() {
                    if stopping.load(Ordering::Acquire) {
                        break;
                    }
                    let Ok(mut stream) = connection else { break };
                    let sender = sender.clone();
                    let pending = clients.clone();
                    let stopping = stopping.clone();
                    let id = id as u64;
                    let _ = std::thread::Builder::new()
                        .name("zvim-launch-client".into())
                        .spawn(move || {
                            let result = (|| -> Result<()> {
                                same_user(&stream)?;
                                stream.set_read_timeout(Some(TIMEOUT))?;
                                stream.set_write_timeout(Some(TIMEOUT))?;
                                let message: Message = read_message(&mut stream)?;
                                let launch = message.launch()?;
                                let (response, replies) = mpsc::channel();
                                {
                                    let mut clients = pending.0.lock().unwrap();
                                    if stopping.load(Ordering::Acquire) {
                                        bail!("Zvim is closing; retry the launch");
                                    }
                                    clients.insert(id, response.clone());
                                }
                                sender.send_blocking(Incoming { launch, response })?;
                                let reply = replies.recv()?;
                                let opened = matches!(reply, Reply::Opened);
                                write_message(&mut stream, &reply)?;
                                if message.wait && opened {
                                    write_message(&mut stream, &replies.recv()?)?;
                                }
                                Ok(())
                            })();
                            if let Err(error) = result {
                                let _ =
                                    write_message(&mut stream, &Reply::Error(error.to_string()));
                            }
                            pending.0.lock().unwrap().remove(&id);
                            pending.1.notify_all();
                        });
                }
            })?;
        Ok(Role::Primary(Self {
            requests,
            stop,
            thread: Some(thread),
            socket,
            pending,
            _directory: directory,
            _lock: lock,
        }))
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = UnixStream::connect(&self.socket);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let mut clients = self.pending.0.lock().unwrap();
        for response in clients.values() {
            let _ = response.send(Reply::Closed);
        }
        while !clients.is_empty() {
            clients = self.pending.1.wait(clients).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn host(dir: &Path) -> Server {
        match Server::acquire(dir).unwrap() {
            Role::Primary(server) => server,
            _ => panic!("expected primary"),
        }
    }
    fn request(wait: bool) -> Message {
        Message::new(
            &Launch {
                working_directory: Some(PathBuf::from("/tmp")),
                ..Default::default()
            },
            wait,
        )
        .unwrap()
    }
    #[test]
    fn second_launch_reuses_primary_and_stale_endpoint_recovers() {
        let dir = tempfile::tempdir().unwrap();
        let server = host(dir.path());
        let stream = match Server::acquire(dir.path()).unwrap() {
            Role::Secondary(stream) => stream,
            _ => panic!("duplicate primary"),
        };
        let client = std::thread::spawn(move || forward(stream, &request(false)));
        let incoming = server.requests.recv_blocking().unwrap();
        let completion = incoming.opened();
        client.join().unwrap().unwrap();
        drop(completion);
        drop(server);
        let server = host(dir.path());
        assert!(connect(dir.path()).is_ok());
        drop(server);
    }
    #[test]
    fn wait_is_per_window_and_shutdown_completes_remaining_waiters() {
        let dir = tempfile::tempdir().unwrap();
        let server = host(dir.path());
        let (finished, outcomes) = mpsc::channel();
        let mut completions = Vec::new();
        for index in 0..2 {
            let stream = connect(dir.path()).unwrap();
            let finished = finished.clone();
            std::thread::spawn(move || {
                finished
                    .send((index, forward(stream, &request(true)).is_ok()))
                    .unwrap();
            });
            completions.push(server.requests.recv_blocking().unwrap().opened());
        }
        assert!(outcomes.try_recv().is_err());
        drop(completions.remove(0));
        assert_eq!(outcomes.recv_timeout(TIMEOUT).unwrap(), (0, true));
        assert!(outcomes.try_recv().is_err());
        drop(server);
        assert_eq!(outcomes.recv_timeout(TIMEOUT).unwrap(), (1, true));
        drop(completions);
    }
    #[test]
    fn wire_preserves_non_utf8_arguments_environment_and_rejects_invalid_input() {
        let mut message = request(false);
        message.args = vec![
            b"-u".to_vec(),
            b"config with spaces".to_vec(),
            vec![b'f', 255],
        ];
        message.environment = vec![(b"CUSTOM".to_vec(), vec![255])];
        let decoded: Message =
            serde_json::from_slice(&serde_json::to_vec(&message).unwrap()).unwrap();
        let launch = decoded.launch().unwrap();
        assert_eq!(launch.nvim_args[2].as_bytes(), [b'f', 255]);
        assert_eq!(launch.environment.unwrap()[0].1.as_bytes(), [255]);
        message.version = 999;
        assert!(message.launch().is_err());
        let (mut a, mut b) = UnixStream::pair().unwrap();
        a.write_all(&((LIMIT + 1) as u32).to_be_bytes()).unwrap();
        assert!(read_message::<Message>(&mut b).is_err());
    }
    #[test]
    fn failed_window_open_is_reported_to_launcher() {
        let dir = tempfile::tempdir().unwrap();
        let server = host(dir.path());
        let stream = connect(dir.path()).unwrap();
        let client = std::thread::spawn(move || forward(stream, &request(false)));
        server
            .requests
            .recv_blocking()
            .unwrap()
            .reject("window failure".into());
        assert!(
            client
                .join()
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("window failure")
        );
    }
}
