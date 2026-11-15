//! What the model must return for a summary to replace a conversation, and
//! what its refusal means to the consumer that has to react to it.

use agent::{ModelResponse, Outcome, StopReason};

/// Summarizing ran but produced nothing usable. A provider failure keeps the
/// provider's own code instead, so a throttle never hides behind this one.
pub const CODE_COMPACTION_FAILED: &str = "compaction_failed";

/// The one shape a summary may take: settled, terminal, and text only.
/// Blank is not text: compaction is destructive, so a whitespace-only summary
/// would hide the whole conversation from the model behind two spaces.
pub(super) fn usable(response: &ModelResponse) -> Option<String> {
    let acceptable = !response.text.trim().is_empty()
        && response.tool_calls.is_empty()
        && matches!(response.stop, StopReason::EndTurn);
    acceptable.then(|| response.text.clone())
}

/// Failures a second attempt moments later could plausibly survive: a stream
/// that broke, not a service that refused. Both provider crates emit this
/// shared slug vocabulary. `rate_limited` is deliberately absent — the provider
/// seam already waits a throttle out properly, honouring `retry-after` on the
/// HTTP path and 1 to 20 seconds of SDK backoff on Bedrock, so one that reaches
/// here needs far longer than this loop waits, and repeating a thread-sized
/// request into a rate limiter adds load without changing the answer. A refused
/// request, bad credentials, or an unusable response answer the same way every
/// time, and an unknown code is left alone rather than guessed at.
const TRANSIENT_CODES: [&str; 3] = ["provider_unavailable", "request_timeout", "transport_error"];

/// Whether another attempt could plausibly answer differently. A response that
/// arrived and was unusable would exhaust the same budget again.
pub(super) fn transient(response: &ModelResponse) -> bool {
    let StopReason::Error { code, .. } = &response.stop else {
        return false;
    };
    TRANSIENT_CODES.contains(&code.as_str())
}

/// Why no summary came back, in the code its consumer branches on. A provider
/// failure reports exactly what a regular turn would, so throttling reads as
/// throttling rather than as a summarization defect; the stage is named in the
/// message, which is for humans and never the branching signal.
pub(super) fn rejected(response: ModelResponse) -> Outcome {
    match response.stop {
        StopReason::Error { code, message } => Outcome::Failed {
            code,
            message: format!("compacting the thread: {message}"),
        },
        stop => Outcome::Failed {
            code: CODE_COMPACTION_FAILED.to_owned(),
            message: format!(
                "summary stopped at {stop:?} with {} characters and {} tool calls",
                response.text.chars().count(),
                response.tool_calls.len()
            ),
        },
    }
}
