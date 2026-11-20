//! Blocking stderr must never block the execution thread or process shutdown.
use std::{
    io::{self, Write},
    sync::{atomic::Ordering, mpsc},
    time::{Duration, Instant},
};
use tracing_subscriber::fmt::MakeWriter as _;
struct Blocked {
    entered: Option<mpsc::Sender<()>>,
    release: mpsc::Receiver<()>,
}
impl Write for Blocked {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(entered) = self.entered.take() {
            entered.send(()).unwrap();
            self.release.recv().unwrap();
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn blocked_stderr_drops_records_and_cannot_hold_shutdown() {
    let (entered, wait) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let output = Blocked {
        entered: Some(entered),
        release: blocked,
    };
    let (writer, worker) = crate::logging_worker::start(output).unwrap();
    writer.make_writer().write_all(b"first\n").unwrap();
    wait.recv_timeout(Duration::from_secs(1)).unwrap();
    let started = Instant::now();
    for _ in 0..2_000 {
        writer.make_writer().write_all(b"event\n").unwrap();
    }
    assert!(writer.dropped.load(Ordering::Relaxed) > 0);
    worker.shutdown();
    assert!(started.elapsed() < Duration::from_secs(1));
    release.send(()).unwrap();
}
#[test]
fn oversized_records_are_dropped_whole() {
    let (writer, worker) = crate::logging_worker::start(io::sink()).unwrap();
    let mut record = writer.make_writer();
    record.write_all(&vec![b'x'; 32 * 1024]).unwrap();
    record.write_all(b"tail").unwrap();
    drop(record);
    assert_eq!(writer.dropped.load(Ordering::Relaxed), 1);
    worker.shutdown();
}
