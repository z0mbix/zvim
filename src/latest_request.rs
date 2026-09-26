use std::sync::{Arc, Mutex, mpsc};

/// Keep at most one request in flight and one replaceable pending value.
pub struct LatestRequest<T> {
    latest: Arc<Mutex<Option<T>>>,
    wake: mpsc::SyncSender<()>,
}
impl<T: Send + 'static> LatestRequest<T> {
    pub fn new(mut apply: impl FnMut(T) -> bool + Send + 'static) -> Self {
        let latest = Arc::new(Mutex::new(None));
        let pending = latest.clone();
        let (wake, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            while receiver.recv().is_ok() {
                let value = pending.lock().unwrap().take();
                if let Some(value) = value
                    && !apply(value)
                {
                    break;
                }
            }
        });
        Self { latest, wake }
    }
    pub fn submit(&self, value: T) {
        *self.latest.lock().unwrap() = Some(value);
        let _ = self.wake.try_send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn slow_consumer_skips_intermediate_values_and_keeps_final_size() {
        let (sent, received) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let queue = LatestRequest::new(move |value| {
            sent.send(value).unwrap();
            wait.recv().is_ok()
        });
        queue.submit((80, 24));
        assert_eq!(
            received.recv_timeout(Duration::from_secs(1)).unwrap(),
            (80, 24)
        );
        for width in 81..=200 {
            queue.submit((width, 40));
        }
        assert!(received.try_recv().is_err());
        release.send(()).unwrap();
        assert_eq!(
            received.recv_timeout(Duration::from_secs(1)).unwrap(),
            (200, 40)
        );
        drop(queue);
        release.send(()).unwrap();
        assert!(received.recv_timeout(Duration::from_secs(1)).is_err());
    }
}
