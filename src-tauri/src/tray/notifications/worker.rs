//! Serialize potentially blocking notification calls without waiting on the UI.

use std::sync::mpsc::{self, SyncSender, TrySendError};

type Job = Box<dyn FnOnce() + Send + 'static>;

pub(super) struct Worker {
    sender: SyncSender<Job>,
}

impl Worker {
    pub(super) fn new(name: &str, capacity: usize) -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel::<Job>(capacity);
        std::thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                for job in receiver {
                    job();
                }
            })
            .map_err(|error| format!("Failed to start notification worker: {error}"))?;
        Ok(Self { sender })
    }

    pub(super) fn submit(&self, job: impl FnOnce() + Send + 'static) -> Result<(), String> {
        // Never wait for a stalled notification service, even when the queue
        // is full. Bound pending work rather than spawning unlimited threads.
        self.sender
            .try_send(Box::new(job))
            .map_err(|error| match error {
                TrySendError::Full(_) => "Windows notification queue is full".to_string(),
                TrySendError::Disconnected(_) => "Windows notification worker stopped".to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const TIMEOUT: Duration = Duration::from_secs(2);

    #[test]
    fn blocked_notification_does_not_block_submitter_and_preserves_order() {
        let worker = Worker::new("notification-test", 2).unwrap();
        let caller_thread = std::thread::current().id();
        let (started, started_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let (finished, finished_rx) = mpsc::channel();
        let first_finished = finished.clone();
        worker
            .submit(move || {
                started.send(std::thread::current().id()).unwrap();
                release_rx.recv().unwrap();
                first_finished.send(1).unwrap();
            })
            .unwrap();
        assert_ne!(started_rx.recv_timeout(TIMEOUT).unwrap(), caller_thread);

        // The first notification is still blocked. A later connection event
        // must return immediately and remain queued behind it.
        worker
            .submit(move || {
                finished.send(2).unwrap();
            })
            .unwrap();
        assert_eq!(finished_rx.try_recv(), Err(mpsc::TryRecvError::Empty));
        release.send(()).unwrap();
        assert_eq!(finished_rx.recv_timeout(TIMEOUT).unwrap(), 1);
        assert_eq!(finished_rx.recv_timeout(TIMEOUT).unwrap(), 2);
    }

    #[test]
    fn full_queue_rejects_work_without_waiting_for_blocked_notification() {
        let worker = Worker::new("notification-full-test", 1).unwrap();
        let (started, started_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        worker
            .submit(move || {
                started.send(()).unwrap();
                release_rx.recv().unwrap();
            })
            .unwrap();
        started_rx.recv_timeout(TIMEOUT).unwrap();
        worker.submit(|| {}).unwrap();
        let result = worker.submit(|| panic!("rejected job must not run"));
        release.send(()).unwrap();
        assert_eq!(
            result,
            Err("Windows notification queue is full".to_string())
        );
    }
}
