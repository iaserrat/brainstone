use std::{pin::Pin, sync::Arc};

use agent::{Model, Tool, ToolError, ToolOutput, ToolRequest};
use runtime::{Store, SubagentId, SubagentStore};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::Control;

pub(crate) struct TaskStopTool<S, M>(pub(crate) Arc<Control<S, M>>);

#[derive(Deserialize)]
struct Args {
    agent_id: i64,
}

impl<S: Store + SubagentStore + 'static, M: Model + 'static> Tool for TaskStopTool<S, M> {
    fn name(&self) -> &'static str {
        "TaskStop"
    }

    fn definition(&self) -> Value {
        json!({
            "name":"TaskStop",
            "description":"Stop a subagent and fence its active durable writer.",
            "input_schema":{"type":"object","required":["agent_id"],"properties":{
                "agent_id":{"type":"integer"}
            }}
        })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            let args: Args = serde_json::from_value(request.args)
                .map_err(|error| ToolError::new("invalid_agent_arguments", error.to_string()))?;
            let context = request.context.ok_or_else(|| {
                ToolError::new(
                    "durable_context_required",
                    "TaskStop requires durable context",
                )
            })?;
            let id = SubagentId::new(args.agent_id).map_err(|error| runtime_error(&error))?;
            let caller = operation_id(context.operation_id)?;
            let agent = self
                .0
                .runtime
                .stop_subagent(caller, id)
                .await
                .map_err(|error| runtime_error(&error))?;
            Ok(json!({"agent_id":agent.id.get(),"state":agent.operation.state,"outcome":agent.operation.outcome}).to_string().into())
        })
    }
}

fn runtime_error(error: &runtime::Error) -> ToolError {
    ToolError::new(error.code(), error.to_string())
}

fn operation_id(value: u64) -> Result<runtime::OperationId, ToolError> {
    let value = i64::try_from(value)
        .map_err(|_| ToolError::new("durable_context_required", "invalid operation id"))?;
    runtime::OperationId::new(value).map_err(|error| runtime_error(&error))
}
