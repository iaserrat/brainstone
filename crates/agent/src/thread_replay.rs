use std::borrow::Cow;

use super::{Message, Thread};
use crate::step::CODE_UNANSWERED_CALL;
use crate::{ModelResponse, StopReason, ToolCall, ToolResult};

#[path = "thread_replay_build.rs"]
mod build;
use build::Replay;

const UNANSWERED: &str = "not executed: the run ended before this call was answered.";

impl Thread {
    /// This thread as `producer` may be shown it: no failed response, no
    /// reasoning another or an unknown model wrote, and every tool call
    /// answered. Borrowed when nothing changes, so a model's own history
    /// replays byte-identical and its prompt cache stays valid.
    #[must_use]
    pub fn replay(&self, producer: &str) -> Cow<'_, Self> {
        let next = self.messages.iter().skip(1).map(Some).chain([None]);
        let mut pairs = self.messages.iter().zip(next);
        if pairs.all(|(message, next)| replayable(message, next, producer)) {
            return Cow::Borrowed(self);
        }
        let mut replay = Replay::default();
        for message in &self.messages {
            replay.push(message, producer);
        }
        replay.close();
        let mut thread = self.clone();
        thread.messages = replay.messages;
        Cow::Owned(thread)
    }
}

fn replayable(message: &Message, next: Option<&Message>, producer: &str) -> bool {
    let Message::Assistant(response) = message else {
        return true;
    };
    let answered = match next {
        Some(Message::ToolResults(results)) => unanswered(&response.tool_calls, results).is_empty(),
        _ => response.tool_calls.is_empty(),
    };
    !failed(response) && (response.reasoning.is_empty() || own(response, producer)) && answered
}

const fn failed(response: &ModelResponse) -> bool {
    matches!(response.stop, StopReason::Error { .. })
}

fn own(response: &ModelResponse, producer: &str) -> bool {
    response.model.as_deref() == Some(producer)
}

/// An error result for each call in `calls` that `results` leaves open.
fn unanswered(calls: &[ToolCall], results: &[ToolResult]) -> Vec<ToolResult> {
    calls
        .iter()
        .filter(|call| !results.iter().any(|result| result.id == call.id))
        .map(|call| crate::call::failed(call, CODE_UNANSWERED_CALL, UNANSWERED))
        .collect()
}
