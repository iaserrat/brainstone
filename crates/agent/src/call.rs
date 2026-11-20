//! Turning tool calls into tool results.
//! Every path out of here produces a result for every call it was given.
//! Providers reject a batch with an unanswered call, so "no result" is not an
//! option the loop can take, whatever went wrong.
//!
//! Known ceiling: a batch runs sequentially. Providers emit batches meant to
//! run concurrently, so this costs wall time on multi-call turns. Fixing it
//! changes [`call_all`] alone and no data.

use serde_json::json;

use crate::event::{Event, Sink};
use crate::model::ToolCall;
use crate::registry::Registry;
use crate::step::CODE_TRUNCATED_CALL;
use crate::tool::ToolResult;
use crate::tool_output::ToolOutput;

#[path = "call_execute.rs"]
mod execute;
pub use execute::{execute_tool, execute_tool_with_context, execute_tool_with_media};

const TRUNCATED: &str = "not executed: the response hit the output token limit, \
    so these arguments may be incomplete. Reissue the call.";

/// Every call of one batch, answered in the order the model asked.
pub(crate) async fn call_all<S: Sink>(
    calls: &[ToolCall],
    tools: &Registry,
    sink: &mut S,
) -> Result<Vec<ToolResult>, S::Error> {
    let mut results = Vec::with_capacity(calls.len());
    for call in calls {
        sink.emit(Event::ToolStarted {
            id: call.id.clone(),
            name: call.name.clone(),
            args: call.args.clone(),
        })
        .await?;
        let result = execute_tool(call, tools, None).await;
        sink.emit(Event::ToolFinished {
            result: result.progress(),
        })
        .await?;
        results.push(result);
    }
    Ok(results)
}

/// The answer to a call that was never run because the response carrying it
/// was cut off. It never reaches a tool, so it emits no start or finish.
#[must_use]
pub fn refused(call: &ToolCall) -> ToolResult {
    failed(call, CODE_TRUNCATED_CALL, TRUNCATED)
}

/// A failure as the model reads it: a structured document carrying the stable
/// code, never prose it would have to parse to find one.
pub(crate) fn failed(call: &ToolCall, code: &str, message: &str) -> ToolResult {
    let message = crate::tool_bound::bounded(message.to_owned());
    let document = json!({ "error": code, "message": message }).to_string();
    ToolOutput::from(document).answer(call, true).0
}
