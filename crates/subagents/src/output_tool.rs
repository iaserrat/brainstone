use std::{pin::Pin, sync::Arc, time::Duration};

use agent::{Model, Tool, ToolError, ToolOutput, ToolRequest};
use runtime::{Store, SubagentId, SubagentStore};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::Control;

pub(crate) struct TaskOutputTool<S, M>(pub(crate) Arc<Control<S, M>>);

#[derive(Deserialize)]
struct Args {
    agent_id: i64,
    #[serde(default = "default_block")]
    block: bool,
    timeout_seconds: Option<u64>,
}

impl<S: Store + SubagentStore + 'static, M: Model + 'static> Tool for TaskOutputTool<S, M> {
    fn name(&self) -> &'static str {
        "TaskOutput"
    }

    fn definition(&self) -> Value {
        json!({
            "name":"TaskOutput",
            "description":"Read a subagent's durable status or wait for its current task to finish.",
            "input_schema":{"type":"object","required":["agent_id"],"properties":{
                "agent_id":{"type":"integer"},"block":{"type":"boolean"},
                "timeout_seconds":{"type":"integer","minimum":1,"maximum":300}
            }}
        })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move { self.output(request).await.map(ToolOutput::from) })
    }
}

impl<S: Store + SubagentStore + 'static, M: Model + 'static> TaskOutputTool<S, M> {
    async fn output(&self, request: ToolRequest) -> Result<String, ToolError> {
        let args: Args = serde_json::from_value(request.args)
            .map_err(|error| ToolError::new("invalid_agent_arguments", error.to_string()))?;
        let operation = request.context.ok_or_else(context_error)?.operation_id;
        let id = SubagentId::new(args.agent_id).map_err(|error| runtime_error(&error))?;
        let caller = operation_id(operation)?;
        let timeout = args.timeout_seconds.unwrap_or(30);
        if !(1..=300).contains(&timeout) {
            return Err(ToolError::new(
                "invalid_agent_arguments",
                "timeout_seconds must be between 1 and 300",
            ));
        }
        self.wait(caller, id, args.block, Duration::from_secs(timeout))
            .await
    }
}

fn context_error() -> ToolError {
    ToolError::new(
        "durable_context_required",
        "TaskOutput requires durable context",
    )
}

const fn default_block() -> bool {
    true
}

pub(crate) fn runtime_error(error: &runtime::Error) -> ToolError {
    ToolError::new(error.code(), error.to_string())
}

pub(crate) fn operation_id(value: u64) -> Result<runtime::OperationId, ToolError> {
    let value = i64::try_from(value)
        .map_err(|_| ToolError::new("durable_context_required", "invalid operation id"))?;
    runtime::OperationId::new(value).map_err(|error| runtime_error(&error))
}
