use agent::ToolError;
use rmcp::{
    RoleClient,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, DEFAULT_MRTR_MAX_ROUNDS,
        InputRequiredResult,
    },
    service::RunningService,
};

use crate::handler::Handler;

pub(crate) async fn execute(
    service: &RunningService<RoleClient, Handler>,
    mut params: CallToolRequestParams,
    lifecycle: &mut crate::tool_lifecycle::Call<'_>,
) -> Result<CallToolResult, ToolError> {
    for _ in 0..DEFAULT_MRTR_MAX_ROUNDS {
        match lifecycle.execute(params.clone()).await? {
            CallToolResponse::Complete(result) => return Ok(result),
            CallToolResponse::InputRequired(required) => {
                apply(service, &mut params, required).await?;
            }
            CallToolResponse::Task(task) => {
                lifecycle.task_id = Some(task.task.task_id.clone());
                return crate::tool_task::poll(service, task.task, params.meta.as_ref()).await;
            }
            _ => return Err(ToolError::new("mcp_tool_call", "unknown response type")),
        }
    }
    Err(ToolError::new(
        "mcp_input_rounds_exceeded",
        "server requested too many input rounds",
    ))
}

async fn apply(
    service: &RunningService<RoleClient, Handler>,
    params: &mut CallToolRequestParams,
    required: InputRequiredResult,
) -> Result<(), ToolError> {
    params.input_responses = match required.input_requests {
        Some(requests) => Some(crate::input::fulfill(service.service(), requests).await?),
        None => None,
    };
    params.request_state = required.request_state;
    Ok(())
}
