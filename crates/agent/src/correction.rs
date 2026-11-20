use crate::{Message, Model, StopReason, Thread};

/// One correction fits the existing response budget and never recursively repairs
/// its own result. Replay derives the bound from typed messages, not process state.
pub async fn output_feedback<M: Model>(model: &M, thread: &Thread) -> Option<String> {
    let Message::Assistant(response) = thread.last() else {
        return None;
    };
    if contract_failed(thread)
        || response.stop != StopReason::EndTurn
        || !response.tool_calls.is_empty()
    {
        return None;
    }
    if thread
        .profile()
        .max_turns()
        .is_some_and(|limit| thread.steps() >= limit)
    {
        return None;
    }
    if already_corrected(thread) {
        return None;
    }
    model.output_feedback(thread).await
}

fn contract_failed(thread: &Thread) -> bool {
    matches!(crate::next(thread), crate::Step::Done(crate::Outcome::Failed { code, .. })
        if code == crate::CODE_TOOL_REQUIREMENT_UNMET || code == crate::CODE_TOOL_EVIDENCE_UNAVAILABLE)
}

fn already_corrected(thread: &Thread) -> bool {
    thread
        .messages()
        .iter()
        .rev()
        .nth(1)
        .is_some_and(|message| matches!(message, Message::OutputCorrection(_)))
}
