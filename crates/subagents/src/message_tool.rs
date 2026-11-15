use std::{pin::Pin, sync::Arc};

use agent::{Model, Tool, ToolError, ToolOutput, ToolRequest};
use runtime::{Store, SubagentId, SubagentStore};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::Control;

pub(crate) struct SendMessageTool<S, M>(pub(crate) Arc<Control<S, M>>);

#[derive(Deserialize)]
struct Args {
    to: i64,
    message: String,
}

impl<S: Store + SubagentStore + 'static, M: Model + 'static> Tool for SendMessageTool<S, M> {
    fn name(&self) -> &'static str {
        "SendMessage"
    }

    fn definition(&self) -> Value {
        json!({
            "name":"SendMessage",
            "description":"Send a durable follow-up to an existing subagent, resuming it with its full context.",
            "input_schema":{"type":"object","required":["to","message"],"properties":{
                "to":{"type":"integer"},"message":{"type":"string"}
            }}
        })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move { self.send(request).await.map(ToolOutput::from) })
    }
}

impl<S: Store + SubagentStore + 'static, M: Model + 'static> SendMessageTool<S, M> {
    async fn send(&self, request: ToolRequest) -> Result<String, ToolError> {
        let args: Args = serde_json::from_value(request.args)
            .map_err(|error| ToolError::new("invalid_agent_arguments", error.to_string()))?;
        let context = request.context.ok_or_else(context_error)?;
        let key = request.idempotency_key.ok_or_else(key_error)?;
        let id = SubagentId::new(args.to).map_err(|error| runtime_error(&error))?;
        let caller = operation_id(context.operation_id)?;
        let agent = self
            .0
            .runtime
            .message_subagent(caller, id, &key, &args.message)
            .await
            .map_err(|error| runtime_error(&error))?;
        Ok(
            json!({"agent_id":agent.id.get(),"operation_id":agent.operation.id.get(),
            "status":agent.operation.state})
            .to_string(),
        )
    }
}

fn context_error() -> ToolError {
    ToolError::new(
        "durable_context_required",
        "SendMessage requires durable context",
    )
}

fn key_error() -> ToolError {
    ToolError::new(
        "durable_context_required",
        "SendMessage requires an idempotency key",
    )
}

fn operation_id(value: u64) -> Result<runtime::OperationId, ToolError> {
    let value = i64::try_from(value)
        .map_err(|_| ToolError::new("durable_context_required", "invalid operation id"))?;
    runtime::OperationId::new(value).map_err(|error| runtime_error(&error))
}

fn runtime_error(error: &runtime::Error) -> ToolError {
    ToolError::new(error.code(), error.to_string())
}
