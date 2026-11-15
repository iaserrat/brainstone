//! The driver. It performs what [`next`] decided and records what that
//! produced; it decides nothing itself. Keeping every branch here a one-liner
//! is what keeps the policy in one pure function rather than spread across the
//! effects that carry it out.

use crate::call::{call_all, refused};
use crate::event::{Event, Sink};
use crate::next::next;
use crate::provider::Model;
use crate::registry::Registry;
use crate::step::{Outcome, Step};
use crate::thread::{Message, Thread};

/// Drive `thread` to an outcome.
///
/// The thread is advanced in place, so a caller that resumes hands back what
/// it last held. Messages already in it are the caller's to have recorded;
/// this only records what it appends.
///
/// # Errors
///
/// Only when the sink refuses a write, and then with the sink's own error. A
/// provider or tool failure is not an error here: it is an [`Outcome`] or a
/// tool result, because the model is meant to read it and carry on.
pub async fn run<M: Model, S: Sink + Send>(
    thread: &mut Thread,
    model: &M,
    tools: &Registry,
    sink: &mut S,
) -> Result<Outcome, S::Error> {
    sink.emit(Event::RunStarted).await?;
    let outcome = drive(thread, model, tools, sink).await?;
    sink.emit(Event::RunFinished {
        outcome: outcome.clone(),
    })
    .await?;
    Ok(outcome)
}

async fn drive<M: Model, S: Sink + Send>(
    thread: &mut Thread,
    model: &M,
    tools: &Registry,
    sink: &mut S,
) -> Result<Outcome, S::Error> {
    loop {
        let message = match next(thread) {
            Step::Done(outcome) => {
                let Some(feedback) = crate::output_feedback(model, thread).await else {
                    return Ok(outcome);
                };
                Message::OutputCorrection(feedback)
            }
            Step::CallModel => {
                let response =
                    crate::observation::generate_stream(model, thread, &tools.definitions(), sink)
                        .await?;
                Message::Assistant(response)
            }
            Step::RunTools(calls) => Message::ToolResults(call_all(&calls, tools, sink).await?),
            Step::FailTruncated(calls) => Message::ToolResults(calls.iter().map(refused).collect()),
        };
        append(thread, message, sink).await?;
    }
}

/// Record first, then append. A store that dies mid-run is then ahead of
/// memory rather than behind it: replay can drop a duplicate, but it cannot
/// invent a message it never saw.
async fn append<S: Sink>(
    thread: &mut Thread,
    message: Message,
    sink: &mut S,
) -> Result<(), S::Error> {
    sink.emit(Event::Appended {
        message: message.clone(),
    })
    .await?;
    thread.push(message);
    Ok(())
}
