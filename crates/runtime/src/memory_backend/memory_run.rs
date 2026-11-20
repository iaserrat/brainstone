use std::collections::HashMap;
use std::time::Duration;

use agent::Event;

use crate::memory_backend::memory_compaction::Compactions;
use crate::memory_backend::memory_history::History;
use crate::memory_backend::memory_lease::Leases;
use crate::{Claimed, Operation, OperationId, OperationState, Prompt};

#[path = "memory_run_record.rs"]
mod record;
#[path = "memory_run_target.rs"]
mod target;

#[derive(Debug, Default)]
pub(crate) struct Runs {
    prompts: HashMap<OperationId, Prompt>,
    events: HashMap<OperationId, Vec<Event>>,
    leases: Leases,
    compactions: Compactions,
}

impl Runs {
    pub(crate) fn register(&mut self, id: OperationId, prompt: Prompt) {
        self.prompts.insert(id, prompt);
    }

    pub(crate) fn prompt(&self, id: OperationId) -> Option<&Prompt> {
        self.prompts.get(&id)
    }

    pub(crate) fn actor_operations(&self, actor: &str) -> usize {
        self.prompts
            .values()
            .filter(|prompt| prompt.actor.as_deref() == Some(actor))
            .count()
    }

    pub(crate) fn claim(
        &mut self,
        operations: &mut HashMap<OperationId, Operation>,
        worker: &str,
        ttl: Duration,
    ) -> Option<Claimed> {
        self.leases.prepare_claim();
        let id = candidate(operations, &self.leases)?;
        let thread = self.thread(operations, id)?;
        let operation = operations.get_mut(&id).expect("selected operation");
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

fn candidate(operations: &HashMap<OperationId, Operation>, leases: &Leases) -> Option<OperationId> {
    operations
        .values()
        .filter(|operation| can_claim(operation, leases))
        .filter(|operation| leases.session_is_free(operation.session_id))
        .map(|operation| operation.id)
        .min_by_key(|id| id.0)
}

pub(super) fn can_claim(operation: &Operation, leases: &Leases) -> bool {
    operation.state == OperationState::Accepted
        || (operation.state == OperationState::Running && leases.operation_is_free(operation.id))
}
