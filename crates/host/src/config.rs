use std::time::Duration;

use crate::Error;

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(250);
const MIN_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_POLL_INTERVAL: Duration = Duration::from_mins(1);
const DEFAULT_WORKERS: usize = 20;
const MAX_WORKERS: usize = 64;

/// Poll cadence and worker count for a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub(crate) poll_interval: Duration,
    pub(crate) workers: usize,
}

impl Config {
    /// # Errors
    ///
    /// Returns `invalid_input` outside the safe 10 ms to 1 minute range.
    pub fn new(poll_interval: Duration) -> Result<Self, Error> {
        if !(MIN_POLL_INTERVAL..=MAX_POLL_INTERVAL).contains(&poll_interval) {
            return Err(Error::InvalidInput {
                field: "poll_interval",
            });
        }
        Ok(Self {
            poll_interval,
            workers: DEFAULT_WORKERS,
        })
    }

    /// Set the bounded number of concurrent durable operations.
    ///
    /// # Errors
    ///
    /// Returns `invalid_input` outside 1 through 64 workers.
    pub fn with_workers(mut self, workers: usize) -> Result<Self, Error> {
        if !(1..=MAX_WORKERS).contains(&workers) {
            return Err(Error::InvalidInput { field: "workers" });
        }
        self.workers = workers;
        Ok(self)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            poll_interval: DEFAULT_POLL_INTERVAL,
            workers: DEFAULT_WORKERS,
        }
    }
}
