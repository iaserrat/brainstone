use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::{Error, Lease, OperationId, SessionId};

#[derive(Debug, Default)]
pub(crate) struct Leases {
    values: HashMap<OperationId, MemoryLease>,
    next_token: i64,
}

#[derive(Debug)]
struct MemoryLease {
    value: Lease,
    expires_at: Instant,
}

impl Leases {
    pub(crate) fn acquire(
        &mut self,
        operation_id: OperationId,
        session_id: SessionId,
        worker: &str,
        ttl: Duration,
    ) -> Lease {
        self.next_token += 1;
        let value = Lease {
            operation_id,
            session_id,
            worker: worker.to_owned(),
            token: self.next_token,
        };
        self.values.insert(
            operation_id,
            MemoryLease {
                value: value.clone(),
                expires_at: Instant::now() + ttl,
            },
        );
        value
    }

    pub(crate) fn heartbeat(&mut self, lease: &Lease, ttl: Duration) -> Result<(), Error> {
        let Some(current) = self.values.get_mut(&lease.operation_id) else {
            return Err(Error::LeaseLost);
        };
        if current.value != *lease || current.expires_at <= Instant::now() {
            self.values.remove(&lease.operation_id);
            return Err(Error::LeaseLost);
        }
        current.expires_at = Instant::now() + ttl;
        Ok(())
    }

    pub(crate) fn validate(&self, lease: &Lease) -> Result<(), Error> {
        let valid = self
            .values
            .get(&lease.operation_id)
            .is_some_and(|item| item.value == *lease && item.expires_at > Instant::now());
        if valid {
            return Ok(());
        }
        Err(Error::LeaseLost)
    }

    pub(crate) fn release(&mut self, id: OperationId) {
        self.values.remove(&id);
    }

    pub(crate) fn prepare_claim(&mut self) {
        let now = Instant::now();
        self.values.retain(|_, lease| lease.expires_at > now);
    }

    pub(crate) fn operation_is_free(&self, id: OperationId) -> bool {
        !self.values.contains_key(&id)
    }

    pub(crate) fn session_is_free(&self, session_id: SessionId) -> bool {
        self.values
            .values()
            .all(|lease| lease.value.session_id != session_id)
    }
}
