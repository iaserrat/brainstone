use crate::{Error, logging_writer::Writer};
use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::Duration,
};

pub(crate) enum Message {
    Record(Vec<u8>),
    Shutdown(mpsc::Sender<()>),
}
/// The OS worker is detached if stderr stalls; it can never hold Tokio shutdown.
pub(crate) struct Worker {
    sender: SyncSender<Message>,
    dropped: Arc<AtomicUsize>,
}
const DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

pub(crate) fn start(output: impl Write + Send + 'static) -> Result<(Writer, Worker), Error> {
    let (sender, receiver) = mpsc::sync_channel(crate::providers::QUEUE_CAPACITY);
    let dropped = Arc::new(AtomicUsize::new(0));
    let failures = dropped.clone();
    std::thread::Builder::new()
        .name("brainstone-logs".into())
        .spawn(move || run(output, &receiver, &failures))
        .map_err(|_| Error::Configuration)?;
    Ok((
        Writer {
            sender: sender.clone(),
            dropped: dropped.clone(),
        },
        Worker { sender, dropped },
    ))
}
impl Worker {
    pub(crate) fn shutdown(self) {
        if self.dropped.load(Ordering::Relaxed) > 0 {
            tracing::warn!(name: "telemetry_log_queue_full", "log records dropped");
        }
        let (done, wait) = mpsc::channel();
        if self.sender.try_send(Message::Shutdown(done)).is_ok() {
            let _ = wait.recv_timeout(DRAIN_TIMEOUT);
        }
    }
}
fn run(mut output: impl Write, receiver: &Receiver<Message>, dropped: &AtomicUsize) {
    while let Ok(message) = receiver.recv() {
        match message {
            Message::Record(bytes) => {
                if output.write_all(&bytes).is_err() {
                    dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
            Message::Shutdown(done) => {
                drop(output.flush());
                let _ = done.send(());
                return;
            }
        }
    }
}
