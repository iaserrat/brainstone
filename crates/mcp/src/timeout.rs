use std::future::Future;
use std::time::Duration;

use tokio::sync::watch;

pub(crate) async fn unless_waiting<F: Future>(
    future: F,
    duration: Duration,
    mut waiting: watch::Receiver<bool>,
) -> Result<F::Output, ()> {
    tokio::pin!(future);
    let mut remaining = duration;
    let mut active_since = tokio::time::Instant::now();
    let deadline = tokio::time::sleep(duration);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            biased;
            output = &mut future => return Ok(output),
            changed = waiting.changed() => {
                changed.map_err(|_| ())?;
                let now = tokio::time::Instant::now();
                if *waiting.borrow_and_update() {
                    remaining = remaining.saturating_sub(now.duration_since(active_since));
                } else {
                    active_since = now;
                    deadline.as_mut().reset(now + remaining);
                }
            }
            () = &mut deadline, if !*waiting.borrow() => return Err(()),
        }
    }
}
