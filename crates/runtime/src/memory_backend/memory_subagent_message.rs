use crate::memory_backend::memory_state::State;
use crate::memory_backend::memory_subagent_prompt;
use crate::{Error, OperationId, Subagent, SubagentId};

impl State {
    pub(crate) fn message_subagent(
        &mut self,
        caller: OperationId,
        id: SubagentId,
        key: &str,
        message: &str,
    ) -> Result<Subagent, Error> {
        self.authorize(caller, id)?;
        let map_key = self.subagent_key(id)?;
        let request = self.subagents[&map_key].0.clone();
        let prompt = memory_subagent_prompt::message(id, key, message, &request)?;
        let prompt = prompt.with_identity(
            self.runs
                .prompt(request.parent)
                .and_then(|parent| parent.identity.clone()),
        );
        let accepted = self.accept(&prompt, false)?;
        let entry = self.subagents.get_mut(&map_key).expect("located subagent");
        let mut result = entry.1.clone();
        result.operation = accepted.operation;
        if !accepted.duplicate {
            entry.1 = result.clone();
        }
        Ok(result)
    }
}
