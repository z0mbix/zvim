//! One ordered background writer; queued placements coalesce to the newest one.
use crate::settings::Settings;
use std::sync::{Arc, Condvar, Mutex, OnceLock};

#[derive(Default)]
struct State {
    pending: Option<Settings>,
    last_submitted: Option<Settings>,
    busy: bool,
    stopping: bool,
}
pub struct GeometryWriter {
    shared: Arc<(Mutex<State>, Condvar)>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl GeometryWriter {
    pub fn new(write: impl Fn(Settings) -> anyhow::Result<()> + Send + 'static) -> Self {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let thread_shared = shared.clone();
        let worker = std::thread::Builder::new()
            .name("zvim-geometry".into())
            .spawn(move || {
                let (lock, changed) = &*thread_shared;
                loop {
                    let mut state = lock.lock().unwrap();
                    while state.pending.is_none() && !state.stopping {
                        state = changed.wait(state).unwrap();
                    }
                    let Some(settings) = state.pending.take() else {
                        break;
                    };
                    state.busy = true;
                    drop(state);
                    let written = settings.clone();
                    let result = write(settings);
                    let mut state = lock.lock().unwrap();
                    if let Err(error) = result {
                        if state.last_submitted.as_ref() == Some(&written) {
                            state.last_submitted = None;
                        }
                        eprintln!("Cannot save window geometry: {error:#}");
                    }
                    state.busy = false;
                    changed.notify_all();
                }
            })
            .expect("start geometry writer");
        Self {
            shared,
            worker: Some(worker),
        }
    }
    pub fn submit(&self, settings: Settings) {
        let (lock, changed) = &*self.shared;
        let mut state = lock.lock().unwrap();
        if state.last_submitted.as_ref() == Some(&settings) {
            return;
        }
        state.last_submitted = Some(settings.clone());
        state.pending = Some(settings);
        changed.notify_one();
    }
    /// Wait from a background task on normal application quit, never during paint.
    pub fn flush(&self) {
        let (lock, changed) = &*self.shared;
        let mut state = lock.lock().unwrap();
        while state.pending.is_some() || state.busy {
            state = changed.wait(state).unwrap();
        }
    }
}
impl Drop for GeometryWriter {
    fn drop(&mut self) {
        let (lock, changed) = &*self.shared;
        lock.lock().unwrap().stopping = true;
        changed.notify_all();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
static WRITER: OnceLock<GeometryWriter> = OnceLock::new();
pub fn submit(settings: Settings) {
    WRITER
        .get_or_init(|| {
            GeometryWriter::new(|settings| settings.save_to(&crate::settings::data_dir()))
        })
        .submit(settings);
}
pub fn flush() {
    if let Some(writer) = WRITER.get() {
        writer.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_writes_can_retry_the_same_placement() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let attempts = Arc::new(AtomicUsize::new(0));
        let count = attempts.clone();
        let writer = GeometryWriter::new(move |_| {
            if count.fetch_add(1, Ordering::SeqCst) == 0 {
                anyhow::bail!("simulated filesystem failure");
            }
            Ok(())
        });
        writer.submit(Settings::default());
        writer.flush();
        writer.submit(Settings::default());
        writer.flush();
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn slow_writer_does_not_block_submission_and_coalesces_in_order() {
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let writes = Arc::new(Mutex::new(Vec::new()));
        let result = writes.clone();
        let writer = GeometryWriter::new(move |s| {
            if s.width == 1. {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            }
            result.lock().unwrap().push(s.width);
            Ok(())
        });
        writer.submit(Settings {
            width: 1.,
            ..Default::default()
        });
        started_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        for width in 2..=100 {
            writer.submit(Settings {
                width: width as f32,
                ..Default::default()
            });
        }
        release_tx.send(()).unwrap();
        writer.flush();
        writer.submit(Settings {
            width: 100.,
            ..Default::default()
        });
        writer.flush();
        assert_eq!(*writes.lock().unwrap(), vec![1., 100.]);
    }
    #[test]
    fn shutdown_drains_latest_placement_and_atomic_file_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_owned();
        let writer = GeometryWriter::new(move |s| s.save_to(&path));
        let expected = Settings {
            width: 1250.,
            height: 850.,
            x: Some(42.),
            y: Some(99.),
        };
        writer.submit(expected.clone());
        drop(writer);
        let actual: Settings =
            serde_json::from_slice(&std::fs::read(dir.path().join("window.json")).unwrap())
                .unwrap();
        assert_eq!(actual, expected);
    }
}
