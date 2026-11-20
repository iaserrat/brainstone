use crate::{CODE_STRUCTURED_OUTPUT_INVALID, Message, Outcome, Step, StopReason, Thread};

pub(super) fn validate(thread: &Thread, step: Step) -> Step {
    let Step::Done(Outcome::Completed { text }) = &step else {
        return step;
    };
    let Some(schema) = thread.profile().output_schema() else {
        return step;
    };
    if matches!(thread.last(), Message::Assistant(response) if response.stop == StopReason::MaxTokens)
    {
        return Step::Done(Outcome::failed(
            CODE_STRUCTURED_OUTPUT_INVALID,
            "structured output was truncated",
        ));
    }
    match schema.validate(text) {
        Ok(_) => step,
        Err(error) => Step::Done(Outcome::failed(error.code(), error.to_string())),
    }
}
