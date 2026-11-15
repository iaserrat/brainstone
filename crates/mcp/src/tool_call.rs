use agent::{ToolError, ToolOutput, ToolRequest};

use crate::tool::RemoteTool;

#[path = "tool_call_params.rs"]
mod metadata;
use metadata::{call_key, params};

#[tracing::instrument(skip_all, fields(server = %tool.server, tool = %tool.remote_name,
    operation_id = request.context.as_ref().map(|context| context.operation_id)))]
pub(crate) async fn call(tool: &RemoteTool, request: ToolRequest) -> Result<ToolOutput, ToolError> {
    let started = std::time::Instant::now();
    let result = agent::observation::run(
        agent::observation::span(agent::observation::Kind::McpCall),
        Box::pin(execute(tool, request)),
        agent::observation::result_outcome,
    )
    .await;
    let code = result.as_ref().err().map_or("ok", |error| error.code);
    tracing::info!(name: "brainstone.mcp.finished",
        code,
        elapsed_ms = started.elapsed().as_millis(),
        "MCP call finished"
    );
    result
}

async fn execute(tool: &RemoteTool, request: ToolRequest) -> Result<ToolOutput, ToolError> {
    let params = params(tool, request)?;
    let _exclusive = queue(tool).await?;
    tool.session
        .ensure_live()
        .await
        .map_err(|error| unavailable(&error))?;
    let guard = tool.session.service.read().await;
    let service = guard.as_ref().ok_or_else(closed)?;
    service.service().set_call_key(call_key(&params));
    let result = crate::tool_execution::call(
        service,
        params,
        &tool.session.invalidated,
        tool.session.tool_timeout,
    )
    .await?;
    crate::tool_content::output(result)
}

fn closed() -> ToolError {
    ToolError::new("mcp_closed", "connection is closed")
}

fn unavailable(error: &crate::error::Error) -> ToolError {
    ToolError::new(error.code(), error.to_string())
}

async fn queue(tool: &RemoteTool) -> Result<tokio::sync::MutexGuard<'_, ()>, ToolError> {
    let queued = std::time::Instant::now();
    let exclusive = agent::observation::run(
        agent::observation::span(agent::observation::Kind::McpQueue),
        tokio::time::timeout(tool.session.tool_timeout, tool.session.call.lock()),
        agent::observation::result_outcome,
    )
    .await
    .map_err(|_| ToolError::new("mcp_queue_timeout", "server is busy; call was not sent"))?;
    tracing::info!(name: "brainstone.mcp.admitted", tool = %tool.name, queue_ms = queued.elapsed().as_millis(), "MCP call admitted");
    Ok(exclusive)
}
