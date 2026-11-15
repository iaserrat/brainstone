use std::time::Duration;

use agent::ToolError;
use rmcp::{
    RoleClient,
    model::{
        CallToolResult, GetTaskParams, RequestMetaObject, Task, TaskPayload, UpdateTaskParams,
    },
    service::RunningService,
};

use crate::handler::Handler;

pub(crate) async fn poll(
    service: &RunningService<RoleClient, Handler>,
    mut task: Task,
    meta: Option<&RequestMetaObject>,
) -> Result<CallToolResult, ToolError> {
    loop {
        let result = get(service, &task, meta).await?;
        task = result.task.task;
        match result.task.payload {
            TaskPayload::Working => {}
            TaskPayload::InputRequired { input_requests } => {
                let responses = crate::input::fulfill(service.service(), input_requests).await?;
                let mut params = UpdateTaskParams::new(&task.task_id, responses);
                params.meta = meta.cloned();
                service.update_task(params).await.map_err(protocol)?;
            }
            TaskPayload::Completed { result } => return decode(result),
            TaskPayload::Failed { error } => {
                return Err(failure("mcp_task_failed", serde_json::Value::Object(error)));
            }
            TaskPayload::Cancelled => return Err(failure("mcp_task_cancelled", "task cancelled")),
            _ => return Err(failure("mcp_task_protocol", "unknown task status")),
        }
    }
}

async fn get(
    service: &RunningService<RoleClient, Handler>,
    task: &Task,
    meta: Option<&RequestMetaObject>,
) -> Result<rmcp::model::GetTaskResult, ToolError> {
    tokio::time::sleep(interval(task)).await;
    let mut params = GetTaskParams::new(&task.task_id);
    params.meta = meta.cloned();
    service.get_task(params).await.map_err(protocol)
}

fn interval(task: &Task) -> Duration {
    Duration::from_millis(task.poll_interval_ms.unwrap_or(1_000))
}

fn decode(result: serde_json::Map<String, serde_json::Value>) -> Result<CallToolResult, ToolError> {
    serde_json::from_value(serde_json::Value::Object(result))
        .map_err(|error| failure("mcp_task_result", error))
}

fn protocol(error: impl std::fmt::Display) -> ToolError {
    failure("mcp_task_protocol", error)
}

fn failure(code: &'static str, error: impl std::fmt::Display) -> ToolError {
    ToolError::new(code, error.to_string())
}
