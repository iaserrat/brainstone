use std::collections::HashMap;
use std::time::Duration;

use super::{History, Runs, can_claim};
use agent::Thread;

use crate::{Claimed, Operation, OperationId, OperationState};

impl Runs {
    pub(crate) fn claim_operation(
        &mut self,
        operations: &mut HashMap<OperationId, Operation>,
        id: OperationId,
        worker: &str,
        ttl: Duration,
    ) -> Option<Claimed> {
        self.leases.prepare_claim();
        let operation = operations.get(&id)?;
        if blocked(operations, operation)
            || !can_claim(operation, &self.leases)
            || !self.leases.session_is_free(operation.session_id)
        {
            return None;
        }
        self.claim_id(operations, id, worker, ttl)
    }

    fn claim_id(
        &mut self,
        operations: &mut HashMap<OperationId, Operation>,
        id: OperationId,
        worker: &str,
        ttl: Duration,
    ) -> Option<Claimed> {
        let thread = self.thread(operations, id)?;
        let operation = operations.get_mut(&id)?;
        operation.state = OperationState::Running;
        let lease = self.leases.acquire(id, operation.session_id, worker, ttl);
        Some(Claimed {
            identity: self.prompts[&id].identity.clone(),
            operation: operation.clone(),
            thread,
            tool_grant: None,
            lease,
        })
    }
}

impl Runs {
    /// The thread `id` holds through its last recorded event.
    pub(crate) fn thread(
        &self,
        operations: &HashMap<OperationId, Operation>,
        id: OperationId,
    ) -> Option<Thread> {
        let lane_id = operations.get(&id)?.lane_id;
        let span = agent::observation::span(agent::observation::Kind::Recovery);
        span.record("operation_id", id.get());
        let _entered = span.enter();
        let history = History {
            operations,
            prompts: &self.prompts,
            events: &self.events,
            compaction: self.compactions.get(lane_id),
        };
        let thread = crate::memory_history::load(&history, id);
        span.record("outcome", "ok");
        Some(thread)
    }
}

fn blocked(operations: &HashMap<OperationId, Operation>, target: &Operation) -> bool {
    operations.values().any(|operation| {
        operation.lane_id == target.lane_id
            && operation.id.0 < target.id.0
            && matches!(
                operation.state,
                OperationState::Accepted | OperationState::Running
            )
    })
}
