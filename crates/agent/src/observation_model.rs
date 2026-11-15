use super::{Kind, model_outcome, result_outcome, run, span};
use crate::{Model, ModelResponse, Sink, Thread};

/// Observe a logical settled model invocation, including its internal retries.
pub async fn generate<M: Model>(
    model: &M,
    thread: &Thread,
    tools: &[serde_json::Value],
) -> ModelResponse {
    run(
        span(Kind::Model),
        model.generate(thread, tools),
        model_outcome,
    )
    .await
}

/// Observe a logical streaming invocation. Sink failure is a failed invocation;
/// a provider error carried inside a successful Rust result is also failed.
///
/// # Errors
/// Returns the sink failure unchanged.
pub async fn generate_stream<M: Model, S: Sink + Send>(
    model: &M,
    thread: &Thread,
    tools: &[serde_json::Value],
    sink: &mut S,
) -> Result<ModelResponse, S::Error> {
    run(
        span(Kind::Model),
        model.generate_stream(thread, tools, sink),
        |result| {
            result.as_ref().map_or_else(
                |_| result_outcome(result),
                |response| model_outcome(response),
            )
        },
    )
    .await
}
