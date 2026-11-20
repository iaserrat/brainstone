//! One bounded input worker avoids blocking execution or its lease heartbeat.
//! It is detached because an OS stdin read cannot be cancelled. There is at most
//! one per CLI process; it cannot hold Tokio shutdown open after cancellation.
use std::{
    io::{self, Write},
    sync::mpsc,
};
use tokio::sync::oneshot;

type Reply = oneshot::Sender<bool>;
pub(crate) struct Console(mpsc::SyncSender<Reply>);
impl Console {
    pub(crate) fn new() -> Self {
        let (sender, requests) = mpsc::sync_channel::<Reply>(1);
        std::thread::spawn(move || {
            for reply in requests {
                let mut line = String::new();
                let approved = io::stdin()
                    .read_line(&mut line)
                    .is_ok_and(|bytes| bytes > 0)
                    && line.trim().eq_ignore_ascii_case("y");
                // Execution may have ended while stdin was waiting; that is normal.
                let _ = reply.send(approved);
            }
        });
        Self(sender)
    }
    pub(crate) async fn decide(&self) -> Result<bool, super::Error> {
        eprint!("Approve this exact call? [y/N] ");
        io::stderr().flush()?;
        let (send, receive) = oneshot::channel();
        self.0.send(send)?;
        Ok(receive.await?)
    }
}
