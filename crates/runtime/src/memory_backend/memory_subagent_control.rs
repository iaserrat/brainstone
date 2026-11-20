use crate::memory_backend::memory_state::{State, SubagentKey};
use crate::{Error, OperationId, OperationState, Subagent, SubagentId};

impl State {
    pub(crate) fn subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
    ) -> Result<Option<Subagent>, Error> {
        self.authorize(caller, id)?;
        let agent = self
            .subagents
            .values()
            .find(|(_, item, _)| item.id == id)
            .map(|(_, item, _)| item.clone());
        Ok(agent.map(|item| self.current(item)))
    }

    pub(crate) fn stop_subagent(
        &mut self,
        caller: OperationId,
        id: SubagentId,
    ) -> Result<Subagent, Error> {
        self.authorize(caller, id)?;
        let map_key = self.subagent_key(id)?;
        let operation_id = self.subagents[&map_key].1.operation.id;
        let operation = self.operations.get_mut(&operation_id).expect("child");
        if matches!(
            operation.state,
            OperationState::Accepted | OperationState::Running
        ) {
            operation.state = OperationState::Failed;
            operation.outcome = Some(agent::Outcome::Failed {
                code: "agent_cancelled".to_owned(),
                message: "subagent was stopped".to_owned(),
            });
            self.runs.cancel(operation_id);
        }
        Ok(self.current(self.subagents[&map_key].1.clone()))
    }

    pub(crate) fn root_session(&self, operation: OperationId) -> Result<crate::SessionId, Error> {
        let session = self
            .operations
            .get(&operation)
            .map(|item| item.session_id)
            .ok_or(Error::InvalidInput {
                field: "operation_id",
            })?;
        Ok(self
            .subagents
            .values()
            .find(|(_, agent, _)| agent.operation.session_id == session)
            .map_or(session, |(_, _, root)| *root))
    }

    pub(crate) fn subagent_key(&self, id: SubagentId) -> Result<SubagentKey, Error> {
        self.subagents
            .iter()
            .find(|(_, (_, agent, _))| agent.id == id)
            .map(|(key, _)| key.clone())
            .ok_or(Error::InvalidInput { field: "agent_id" })
    }

    pub(crate) fn authorize(&self, caller: OperationId, id: SubagentId) -> Result<(), Error> {
        let root = self
            .subagents
            .values()
            .find(|(_, agent, _)| agent.id == id)
            .filter(|(request, _, _)| {
                self.runs
                    .prompt(request.parent)
                    .map(|prompt| &prompt.identity)
                    == self.runs.prompt(caller).map(|prompt| &prompt.identity)
            })
            .map(|(_, _, root)| *root)
            .ok_or(Error::InvalidInput { field: "agent_id" })?;
        (self.root_session(caller)? == root)
            .then_some(())
            .ok_or(Error::InvalidInput { field: "agent_id" })
    }

    fn current(&self, mut agent: Subagent) -> Subagent {
        agent.operation = self.operations[&agent.operation.id].clone();
        agent
    }
}
