use std::time::Duration;

use agent::Model;
use runtime::Store;

use crate::worker::Cycle;
use crate::{Error, Host};

/// Pauses between one worker's consecutive store failures. A stall shorter
/// than their sum, plus the pool's acquire timeout per attempt, is waited out;
/// a store still failing after the last pause is down, and the worker fails
/// closed so the process supervisor restarts it.
const PAUSES: [Duration; 6] = [
    Duration::from_millis(250),
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
];

impl<S: Store, M: Model> Host<S, M> {
    /// Run one cycle, waiting out store failures and lost leases. The
    /// interrupted operation is left to its lease: once it expires, any worker
    /// resumes it from its records, exactly as after a crash.
    pub(crate) async fn attempt(&self) -> Result<Cycle, Error> {
        let mut failures = 0;
        loop {
            let closed = self.inner.closed.notified();
            tokio::pin!(closed);
            closed.as_mut().enable();
            let error = match self.cycle().await {
                Err(error) => error,
                cycle => return cycle,
            };
            let pause = pause(error, failures)?;
            failures += 1;
            tokio::select! {
                () = &mut closed => {}
                () = tokio::time::sleep(pause) => {}
            }
        }
    }
}

/// The pause after `failures` earlier consecutive failures, or `error` itself
/// when no retry can help. A stall that outlasts the lease makes the next
/// heartbeat or append report `lease_lost`, so it is retried like a store failure.
fn pause(error: Error, failures: usize) -> Result<Duration, Error> {
    let Error::Runtime(runtime::Error::StoreUnavailable { .. } | runtime::Error::LeaseLost) =
        &error
    else {
        return Err(error);
    };
    let Some(&pause) = PAUSES.get(failures) else {
        return Err(error);
    };
    tracing::warn!(name: "brainstone.worker.retry",
        code = error.code(),
        error = %error,
        failures = failures + 1,
        "worker store failure; retrying"
    );
    Ok(pause)
}
