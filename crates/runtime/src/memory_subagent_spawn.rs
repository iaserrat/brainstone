use crate::memory_state::{State, SubagentKey};
use crate::{
    Error, Operation, OperationState, SpawnedSubagent, Subagent, SubagentId, SubagentRequest,
};

const MAX_ACTIVE_SUBAGENTS: usize = 20;

impl State {
    pub(crate) fn spawn_subagent(
        &mut self,
        request: &SubagentRequest,
    ) -> Result<SpawnedSubagent, Error> {
        let key = (request.parent, request.key.clone());
        if let Some(existing) = self.existing_subagent(&key, request)? {
            return Ok(existing);
        }
        if !self.operations.contains_key(&request.parent) {
            return Err(Error::InvalidInput {
                field: "parent_operation_id",
            });
        }
        let root = self.root_session(request.parent)?;
        if self.active_subagents(root) >= MAX_ACTIVE_SUBAGENTS {
            return Err(Error::AgentConcurrencyExceeded);
        }
        self.next_subagent_id += 1;
        let id = SubagentId(self.next_subagent_id);
        let prompt = crate::memory_subagent_prompt::initial(id, request)?;
        let prompt = prompt.with_identity(self.runs.prompt(request.parent).and_then(|parent| parent.identity.clone()));
        let accepted = self.accept(&prompt, false)?;
        let agent = agent(id, accepted.operation, request);
        self.subagents
            .insert(key, (request.clone(), agent.clone(), root));
        Ok(SpawnedSubagent {
            agent,
            duplicate: false,
        })
    }

    fn existing_subagent(
        &self,
        key: &SubagentKey,
        request: &SubagentRequest,
    ) -> Result<Option<SpawnedSubagent>, Error> {
        let Some((stored, agent, _)) = self.subagents.get(key) else {
            return Ok(None);
        };
        if stored != request {
            return Err(Error::IdempotencyConflict);
        }
        Ok(Some(SpawnedSubagent {
            agent: agent.clone(),
            duplicate: true,
        }))
    }

    fn active_subagents(&self, root: crate::SessionId) -> usize {
        self.subagents
            .values()
            .filter(|(_, agent, candidate)| {
                let state = self.operations[&agent.operation.id].state;
                *candidate == root
                    && matches!(state, OperationState::Accepted | OperationState::Running)
            })
            .count()
    }
}

fn agent(id: SubagentId, operation: Operation, request: &SubagentRequest) -> Subagent {
    Subagent {
        id,
        operation,
        name: request.config.name.clone(),
        background: request.config.background,
    }
}
