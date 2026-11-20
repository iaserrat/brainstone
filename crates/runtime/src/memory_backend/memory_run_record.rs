use crate::memory_backend::{memory_finish, memory_steering};
use std::collections::HashMap;
use std::time::Duration;

use agent::Event;

use super::Runs;
use crate::{Compaction, Error, Lease, Operation, OperationId, OperationState};

impl Runs {
    pub(crate) fn has_steering(
        &self,
        lease: &Lease,
        steering: &memory_steering::Steerings,
    ) -> Result<bool, Error> {
        self.leases.validate(lease)?;
        Ok(steering
            .rows
            .values()
            .any(|row| row.operation_id == lease.operation_id && !row.consumed))
    }

    pub(crate) fn record(
        &mut self,
        operations: &mut HashMap<OperationId, Operation>,
        lease: &Lease,
        event: &Event,
    ) -> Result<(), Error> {
        self.leases.validate(lease)?;
        let running = operations
            .get(&lease.operation_id)
            .is_some_and(|item| item.state == OperationState::Running);
        if !running {
            return Err(Error::LeaseLost);
        }
        self.events
            .entry(lease.operation_id)
            .or_default()
            .push(event.clone());
        if let Event::RunFinished { outcome } = event {
            memory_finish::finish(operations, lease.operation_id, outcome)?;
            self.leases.release(lease.operation_id);
        }
        Ok(())
    }

    pub(crate) fn cancel(&mut self, id: OperationId) {
        self.leases.release(id);
    }

    // software-design-allow: pass-through-method keeps lease storage private behind the run boundary.
    pub(crate) fn heartbeat(&mut self, lease: &Lease, ttl: Duration) -> Result<(), Error> {
        self.leases.heartbeat(lease, ttl)
    }

    pub(crate) fn compact(
        &mut self,
        operations: &HashMap<OperationId, Operation>,
        compaction: &Compaction,
    ) -> Result<(), Error> {
        self.compactions.insert(operations, compaction)
    }
}
