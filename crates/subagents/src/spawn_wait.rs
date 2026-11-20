use std::time::Duration;

use agent::Model;
use runtime::{OperationState, Store, SubagentStore};
use serde_json::json;

use crate::{AgentTool, Error};

impl<S: Store + SubagentStore + 'static, M: Model + 'static> AgentTool<S, M> {
    pub(crate) async fn wait(&self, agent: runtime::Subagent) -> Result<String, Error> {
        loop {
            if let Some(result) = self.finished(&agent).await? {
                return Ok(result);
            }
            self.work(agent.operation.id).await?;
        }
    }

    async fn finished(&self, agent: &runtime::Subagent) -> Result<Option<String>, Error> {
        let operation = self
            .0
            .runtime
            .operation(agent.operation.id)
            .await
            .map_err(Error::Runtime)?
            .ok_or(Error::AgentLost)?;
        if !matches!(
            operation.state,
            OperationState::Completed | OperationState::Failed
        ) {
            return Ok(None);
        }
        let output = json!({"agent_id": agent.id.get(), "outcome": operation.outcome});
        serde_json::to_string(&output)
            .map(Some)
            .map_err(|error| Error::Encode {
                message: error.to_string(),
            })
    }

    async fn work(&self, id: runtime::OperationId) -> Result<(), Error> {
        let tools = self.0.registry();
        if self
            .0
            .runtime
            .run_operation(id, &self.0.model, &tools)
            .await
            .map_err(Error::Runtime)?
            .is_none()
        {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }
}
