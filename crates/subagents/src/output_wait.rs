use std::time::Duration;

use agent::{Model, ToolError};
use runtime::{OperationState, Store, SubagentId, SubagentStore};
use serde_json::json;

use crate::output_tool::{TaskOutputTool, runtime_error};

impl<S: Store + SubagentStore + 'static, M: Model + 'static> TaskOutputTool<S, M> {
    pub(crate) async fn wait(
        &self,
        caller: runtime::OperationId,
        id: SubagentId,
        block: bool,
        timeout: Duration,
    ) -> Result<String, ToolError> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let agent = self.read(caller, id).await?;
            if terminal(&agent) || !block || tokio::time::Instant::now() >= deadline {
                return Ok(render(id, &agent));
            }
            self.work(agent.operation.id, deadline).await?;
        }
    }

    async fn read(
        &self,
        caller: runtime::OperationId,
        id: SubagentId,
    ) -> Result<runtime::Subagent, ToolError> {
        self.0
            .runtime
            .subagent(caller, id)
            .await
            .map_err(|error| runtime_error(&error))?
            .ok_or_else(|| ToolError::new("unknown_agent", "agent does not exist"))
    }

    async fn work(
        &self,
        id: runtime::OperationId,
        deadline: tokio::time::Instant,
    ) -> Result<(), ToolError> {
        // Dropping inline execution leaves its journal and lease intact;
        // another worker can resume it after lease expiry.
        match tokio::time::timeout_at(deadline, self.run(id)).await {
            Ok(result) => result,
            Err(_) => Ok(()),
        }
    }

    async fn run(&self, id: runtime::OperationId) -> Result<(), ToolError> {
        let tools = self.0.registry();
        if self
            .0
            .runtime
            .run_operation(id, &self.0.model, &tools)
            .await
            .map_err(|error| runtime_error(&error))?
            .is_none()
        {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }
}

fn terminal(agent: &runtime::Subagent) -> bool {
    matches!(
        agent.operation.state,
        OperationState::Completed | OperationState::Failed
    )
}

fn render(id: SubagentId, agent: &runtime::Subagent) -> String {
    json!({"agent_id":id.get(),"state":agent.operation.state,"outcome":agent.operation.outcome})
        .to_string()
}
