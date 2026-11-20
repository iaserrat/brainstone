use crate::memory_backend::memory_state::State;
use crate::{Error, OperationId, Prompt, SubagentId, SubagentRequest};

impl State {
    pub(crate) fn background_notification(
        &self,
        child: OperationId,
        event: &agent::Event,
    ) -> Result<Option<Prompt>, Error> {
        let agent::Event::RunFinished { outcome } = event else {
            return Ok(None);
        };
        let Some((request, agent, _)) = self
            .subagents
            .values()
            .find(|(request, agent, _)| request.config.background && agent.operation.id == child)
        else {
            return Ok(None);
        };
        let parent = self
            .runs
            .prompt(request.parent)
            .ok_or(Error::InvalidInput {
                field: "parent_operation_id",
            })?;
        let text = serde_json::json!({"type":"subagent_completion","agent_id":agent.id.get(),
            "agent_name":agent.name,"outcome":outcome})
        .to_string();
        let prompt = notification(parent, agent.id, child, &text)?;
        Ok(Some(prompt))
    }
}

fn notification(
    parent: &Prompt,
    id: SubagentId,
    child: OperationId,
    text: &str,
) -> Result<Prompt, Error> {
    let key = format!("subagent-complete/{}/{}", id.get(), child.0);
    let prompt = Prompt::new(parent.address.clone(), &key, &parent.system, text)?
        .with_profile(parent.profile.clone())
        .with_identity(parent.identity.clone());
    Ok(parent
        .delivery
        .clone()
        .map_or(prompt.clone(), |target| prompt.with_delivery(target)))
}

pub(crate) fn initial(id: SubagentId, request: &SubagentRequest) -> Result<Prompt, Error> {
    message(id, "initial", &request.task, request)
}

pub(crate) fn message(
    id: SubagentId,
    key: &str,
    text: &str,
    request: &SubagentRequest,
) -> Result<Prompt, Error> {
    let conversation = format!("agent-{}", id.get());
    let address = crate::Address::new("subagent", &conversation, "main")?;
    Ok(Prompt::new(address, key, &request.config.system, text)?
        .with_profile(request.config.profile.clone()))
}
