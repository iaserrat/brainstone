//! The loop's decision function.
//!
//! Pure: no I/O, no clock, no store. That is what makes every branch
//! table-testable without a provider, and what makes resume free — a restored
//! thread and a live one at the same shape get the same answer.

use crate::model::{ModelResponse, StopReason};
use crate::step::{CODE_STEP_BUDGET, Outcome, SILENCE_TOOL, Step};
use crate::thread::{Message, Thread};
use crate::tool::ToolResult;

/// The one unfinished transition of `thread`.
///
/// The budget is checked here rather than in the driver because this is the
/// only place that knows a model call is what comes next. Tools the model
/// already asked for still run, so the thread is never left holding a batch
/// no result answers.
#[must_use]
pub fn next(thread: &Thread) -> Step {
    match (thread.last(), thread.profile().max_turns()) {
        (Message::Assistant(response), _) => classify(response),
        (Message::ToolResults(results), _) if declined(results) => Step::Done(Outcome::Completed {
            text: String::new(),
        }),
        (_, Some(limit)) if thread.steps() >= limit => Step::Done(Outcome::failed(
            CODE_STEP_BUDGET,
            format!("run reached its budget of {limit} model responses"),
        )),
        _ => Step::CallModel,
    }
}

/// The model called [`SILENCE_TOOL`] and it ran: this turn ends without a
/// reply. An errored call is not that decision — the likeliest cause is a tool
/// the registry does not carry — so the model sees the error and answers.
fn declined(results: &[ToolResult]) -> bool {
    results
        .iter()
        .any(|result| !result.is_error && result.name == SILENCE_TOOL)
}

/// What a settled response means for the run.
fn classify(response: &ModelResponse) -> Step {
    match &response.stop {
        StopReason::Error { code, message } => {
            Step::Done(Outcome::failed(code.clone(), message.clone()))
        }
        StopReason::MaxTokens => truncated(response),
        _ => run_or_finish(response),
    }
}

/// A response cut off by the token limit. Its prose is still worth delivering,
/// but its tool calls are not safe to run: streamed arguments are
/// salvage-parsed, so a call can parse and validate while being incomplete.
fn truncated(response: &ModelResponse) -> Step {
    if response.tool_calls.is_empty() {
        return completed(response);
    }
    Step::FailTruncated(response.tool_calls.clone())
}

/// A response that stopped cleanly. Asking for nothing ends the run, whatever
/// the stop reason claimed — a provider reporting `ToolUse` with no calls would
/// otherwise hang the loop forever.
fn run_or_finish(response: &ModelResponse) -> Step {
    if response.tool_calls.is_empty() {
        return completed(response);
    }
    Step::RunTools(response.tool_calls.clone())
}

fn completed(response: &ModelResponse) -> Step {
    Step::Done(Outcome::Completed {
        text: response.text.clone(),
    })
}
