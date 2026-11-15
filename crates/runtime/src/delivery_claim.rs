use std::time::Duration;

use crate::Error;

/// One target's pending deliveries: what to claim, for whom, and how many.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryClaim {
    pub(crate) adapter: String,
    pub(crate) worker: String,
    pub(crate) ttl: Duration,
    pub(crate) limit: usize,
    pub(crate) presence: Option<Duration>,
}

impl DeliveryClaim {
    /// # Errors
    ///
    /// Returns `invalid_input` for an empty identity, zero TTL, or zero limit.
    pub fn new(adapter: &str, worker: &str, ttl: Duration, limit: usize) -> Result<Self, Error> {
        validate(adapter, worker, ttl, limit)?;
        Ok(Self {
            adapter: adapter.to_owned(),
            worker: worker.to_owned(),
            ttl,
            limit,
            presence: None,
        })
    }

    /// Reclaim a still-running operation after this long with nothing new to say.
    ///
    /// # Errors
    ///
    /// Returns `invalid_input` when the interval is zero.
    pub fn with_presence(mut self, presence: Duration) -> Result<Self, Error> {
        if presence.is_zero() {
            return Err(Error::InvalidInput {
                field: "delivery_presence",
            });
        }
        self.presence = Some(presence);
        Ok(self)
    }
}

fn validate(adapter: &str, worker: &str, ttl: Duration, limit: usize) -> Result<(), Error> {
    let field = if adapter.is_empty() {
        Some("delivery_adapter")
    } else if worker.is_empty() {
        Some("delivery_worker")
    } else if ttl.is_zero() {
        Some("delivery_lease_ttl")
    } else if limit == 0 {
        Some("delivery_batch_limit")
    } else {
        None
    };
    field.map_or(Ok(()), |field| Err(Error::InvalidInput { field }))
}
