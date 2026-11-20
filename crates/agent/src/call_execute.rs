//! Running one call against the registry.

use super::failed;
use crate::File;
use crate::model::ToolCall;
use crate::registry::Registry;
use crate::step::CODE_UNKNOWN_TOOL;
use crate::tool::{ToolContext, ToolError, ToolRequest, ToolResult};
use crate::tool_output::ToolOutput;

/// Execute one call. Durable drivers supply a stable key for safe retries.
pub async fn execute_tool(
    call: &ToolCall,
    tools: &Registry,
    idempotency_key: Option<String>,
) -> ToolResult {
    execute_tool_with_context(call, tools, idempotency_key, None).await
}

/// Execute one call with durable caller context.
pub async fn execute_tool_with_context(
    call: &ToolCall,
    tools: &Registry,
    idempotency_key: Option<String>,
    context: Option<ToolContext>,
) -> ToolResult {
    execute_tool_with_media(call, tools, idempotency_key, context)
        .await
        .0
}

/// Execute one call, handing back beside its result the files it returned:
/// only a durable driver can keep them for the reply, outside the thread.
pub async fn execute_tool_with_media(
    call: &ToolCall,
    tools: &Registry,
    idempotency_key: Option<String>,
    context: Option<ToolContext>,
) -> (ToolResult, Vec<File>) {
    crate::observation::run(
        crate::observation::span(crate::observation::Kind::Tool),
        execute(call, tools, idempotency_key, context),
        |(result, _)| if result.is_error { "failed" } else { "ok" },
    )
    .await
}

async fn execute(
    call: &ToolCall,
    tools: &Registry,
    idempotency_key: Option<String>,
    context: Option<ToolContext>,
) -> (ToolResult, Vec<File>) {
    let Some(tool) = tools.lookup(&call.name) else {
        let unknown = ToolError::new(CODE_UNKNOWN_TOOL, format!("no tool named {}", call.name));
        return answer(call, Err(unknown));
    };
    let request = ToolRequest {
        args: call.args.clone(),
        idempotency_key,
        context,
    };
    answer(call, tool.call(request).await)
}

/// A failed call returns nothing for the reply.
fn answer(call: &ToolCall, outcome: Result<ToolOutput, ToolError>) -> (ToolResult, Vec<File>) {
    match outcome {
        Ok(output) => ToolOutput {
            text: crate::tool_bound::bounded(output.text),
            ..output
        }
        .answer(call, false),
        Err(error) => (failed(call, error.code, &error.message), Vec::new()),
    }
}
