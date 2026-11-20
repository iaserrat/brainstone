use std::time::Duration;

use crate::memory_backend::memory_state::State;
use crate::{Compaction, Error, Lease};

impl State {
    pub(crate) fn record(&mut self, lease: &Lease, event: &agent::Event) -> Result<(), Error> {
        let notification = self.background_notification(lease.operation_id, event)?;
        self.runs.record(&mut self.operations, lease, event)?;
        self.deliveries.record(lease.operation_id, event);
        if let agent::Event::RunFinished { outcome } = event {
            self.schedules.settle(lease.operation_id, outcome)?;
        }
        if let Some(prompt) = notification {
            self.accept(&prompt, false)?;
        }
        Ok(())
    }

    // software-design-allow: pass-through-method keeps run storage private behind the state boundary.
    pub(crate) fn heartbeat(&mut self, lease: &Lease, ttl: Duration) -> Result<(), Error> {
        self.runs.heartbeat(lease, ttl)
    }

    pub(crate) fn compact(&mut self, compaction: &Compaction) -> Result<(), Error> {
        self.runs.compact(&self.operations, compaction)
    }
}
