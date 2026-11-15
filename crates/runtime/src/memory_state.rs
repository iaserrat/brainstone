use std::collections::{HashMap, HashSet};

use crate::memory_address::Addresses;
use crate::memory_delivery::Deliveries;
use crate::memory_run::Runs;
use crate::memory_schedule::Schedules;
use crate::memory_steering::Steerings;
#[path = "memory_state_request.rs"]
mod request;
pub(crate) use request::request_key;

use crate::{
    Accepted, Error, Memory, Operation, OperationId, OperationState, Prompt, Subagent,
    SubagentRequest,
};

pub(crate) type RequestKey = (String, String, String, String);
pub(crate) type SubagentKey = (OperationId, String);
#[derive(Debug, Default)]
pub(crate) struct State {
    pub(crate) next_id: i64,
    pub(crate) operations: HashMap<OperationId, Operation>,
    requests: HashMap<RequestKey, (Prompt, OperationId)>,
    pub(crate) subagents: HashMap<SubagentKey, (SubagentRequest, Subagent, crate::SessionId)>,
    pub(crate) next_subagent_id: i64,
    pub(crate) interactions: HashMap<String, crate::Interaction>,
    pub(crate) addresses: Addresses,
    pub(crate) unfollowed_lanes: HashSet<crate::LaneId>,
    pub(crate) follow_activations: HashSet<(crate::LaneId, String)>,
    pub(crate) unfollow_calls: HashMap<String, crate::LaneId>,
    pub(crate) runs: Runs,
    pub(crate) deliveries: Deliveries,
    pub(crate) steerings: Steerings,
    pub(crate) schedules: Schedules,
    pub(crate) memories: Vec<Memory>,
    /// Tool media by root session and reference. No retention: the store
    /// lives only as long as the process.
    pub(crate) media: HashMap<(crate::SessionId, String), crate::Media>,
}

impl State {
    pub(crate) fn accept(
        &mut self,
        prompt: &Prompt,
        reconfigure: bool,
    ) -> Result<Accepted, Error> {
        let (session_id, lane_id) = self.addresses.resolve(prompt)?;
        self.addresses.configure(lane_id, prompt, reconfigure)?;
        let key = request_key(prompt);
        if let Some(existing) = self.existing(&key, prompt)? {
            return Ok(existing);
        }
        self.next_id += 1;
        let operation = Operation {
            id: OperationId(self.next_id),
            session_id,
            lane_id,
            state: OperationState::Accepted,
            outcome: None,
        };
        self.operations.insert(operation.id, operation.clone());
        self.runs.register(operation.id, prompt.clone());
        self.deliveries
            .register(operation.id, prompt.delivery.clone());
        self.requests.insert(key, (prompt.clone(), operation.id));
        Ok(Accepted {
            operation,
            duplicate: false,
        })
    }

    pub(crate) fn operation(&self, id: OperationId) -> Option<Operation> {
        self.operations.get(&id).cloned()
    }

    pub(crate) fn existing(
        &self,
        key: &RequestKey,
        prompt: &Prompt,
    ) -> Result<Option<Accepted>, Error> {
        let Some((stored, id)) = self.requests.get(key) else {
            return Ok(None);
        };
        if !stored.same_request(prompt) {
            return Err(Error::IdempotencyConflict);
        }
        Ok(Some(Accepted {
            operation: self.operations[id].clone(),
            duplicate: true,
        }))
    }
}
