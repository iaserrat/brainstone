use crate::{handler::Handler, tool_lifecycle::Call};
use agent::ToolError;
use rmcp::{
    RoleClient,
    model::{CallToolRequestParams, CallToolResult},
    service::RunningService,
};
use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

pub(crate) async fn call(
    service: &RunningService<RoleClient, Handler>,
    params: CallToolRequestParams,
    invalidated: &AtomicBool,
    timeout: Duration,
) -> Result<CallToolResult, ToolError> {
    let started = Instant::now();
    let waiting = service.service().waiting();
    let mut lifecycle = Call::new(service, invalidated);
    let future = crate::tool_protocol::execute(service, params, &mut lifecycle);
    let result = crate::timeout::unless_waiting(future, timeout, waiting).await;
    tracing::info!(
        execution_ms = started.elapsed().as_millis(),
        "MCP execution finished"
    );
    if let Ok(result) = result {
        lifecycle.complete();
        result
    } else {
        lifecycle.cancel().await;
        Err(ToolError::new(
            "mcp_tool_timeout",
            "tool call timed out; outcome is unknown; connection will be replaced before the next call; do not blindly retry writes",
        ))
    }
}
