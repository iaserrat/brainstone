use super::Execution;
use crate::{Error, Store};
use agent::{Message, Model, ToolCall, ToolResult};

/// The call was superseded by a steering message and never ran.
pub const CODE_TOOL_SUPERSEDED: &str = "tool_call_superseded";

impl<S: Store, M: Model> Execution<'_, S, M> {
    pub(super) fn push(&mut self, messages: Vec<Message>) {
        for message in messages {
            self.claimed.thread.push(message);
        }
    }

    pub(super) async fn steer_tools(
        &mut self,
        calls: &[ToolCall],
        results: &mut Vec<ToolResult>,
    ) -> Result<bool, Error> {
        if self.store.has_steering(&self.claimed.lease).await? != Some(true) {
            return Ok(false);
        }
        results.extend(calls.iter().skip(results.len()).map(superseded));
        // Close the batch before appending user input. Either committed boundary
        // is independently recoverable; reasoning and call/result pairing stay intact.
        self.append(Message::ToolResults(std::mem::take(results)))
            .await?;
        self.apply_steering().await?;
        Ok(true)
    }
}

fn superseded(call: &ToolCall) -> ToolResult {
    ToolResult {
        id: call.id.clone(), name: call.name.clone(), is_error: true,
        content: serde_json::json!({"error": CODE_TOOL_SUPERSEDED, "message":"Skipped in this attempt because newer user input superseded the call. Earlier attempts may have produced effects; reconcile before retrying."}).to_string(),
        images: Vec::new(),
    }
}
