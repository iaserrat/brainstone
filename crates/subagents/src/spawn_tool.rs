use std::{pin::Pin, sync::Arc};

use agent::{Model, Tool, ToolError, ToolOutput, ToolRequest};
use runtime::{Store, SubagentStore};
use serde_json::{Value, json};

use crate::{Control, Error};

pub(crate) struct AgentTool<S, M>(pub(crate) Arc<Control<S, M>>);

impl<S: Store + SubagentStore + 'static, M: Model + 'static> Tool for AgentTool<S, M> {
    fn name(&self) -> &'static str {
        "Agent"
    }

    fn definition(&self) -> Value {
        let agents: Vec<Value> = self
            .0
            .catalog
            .iter()
            .map(|agent| json!({"name": agent.name(), "description": agent.description()}))
            .collect();
        json!({
            "name": "Agent",
            "description": format!("Delegate to a specialized agent. Available agents: {}", serde_json::to_string(&agents).unwrap()),
            "input_schema": {"type":"object", "required":["subagent_type","prompt"], "properties": {
                "subagent_type":{"type":"string"}, "prompt":{"type":"string"},
                "model":{"type":"string"}, "run_in_background":{"type":"boolean"}
            }}
        })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            self.spawn(request)
                .await
                .map_err(|error| tool_error(&error))
                .map(ToolOutput::from)
        })
    }
}

fn tool_error(error: &Error) -> ToolError {
    ToolError::new(error.code(), error.to_string())
}
