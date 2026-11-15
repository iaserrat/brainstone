use super::logging_worker::Message;
use std::{
    io::{self, Write},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::SyncSender,
    },
};

// A log record contains only bounded platform metadata and operator identity.
const MAX_RECORD_BYTES: usize = 16 * 1024;
#[derive(Clone)]
pub(crate) struct Writer {
    pub(crate) sender: SyncSender<Message>,
    pub(crate) dropped: Arc<AtomicUsize>,
}
pub(crate) struct Record {
    writer: Writer,
    bytes: Vec<u8>,
    oversized: bool,
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Writer {
    type Writer = Record;
    fn make_writer(&'a self) -> Record {
        Record {
            writer: self.clone(),
            bytes: Vec::new(),
            oversized: false,
        }
    }
}
impl Write for Record {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > MAX_RECORD_BYTES {
            self.oversized = true;
        }
        if !self.oversized {
            self.bytes.extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Drop for Record {
    fn drop(&mut self) {
        if self.oversized
            || self
                .writer
                .sender
                .try_send(Message::Record(std::mem::take(&mut self.bytes)))
                .is_err()
        {
            self.writer.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}
