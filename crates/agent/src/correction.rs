use crate::{Message, Model, StopReason, Thread};

/// One correction fits the existing response budget and never recursively repairs
/// its own result. Replay derives the bound from typed messages, not process state.
pub async fn output_feedback<M: Model>(model: &M, thread: &Thread) -> Option<String> {
    let Message::Assistant(response) = thread.last() else {
        return None;
    };
    if response.stop != StopReason::EndTurn || !response.tool_calls.is_empty() {
        return None;
    }
    if thread
        .profile()
        .max_turns()
        .is_some_and(|limit| thread.steps() >= limit)
    {
        return None;
    }
    if thread
        .messages()
        .iter()
        .rev()
        .nth(1)
        .is_some_and(|message| matches!(message, Message::OutputCorrection(_)))
    {
        return None;
    }
    model.output_feedback(thread).await
}
