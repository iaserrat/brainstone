use std::borrow::Cow;
use std::convert::Infallible;

use agent::{Event, ModelResponse, Sink, StopReason, Thread};

/// A model id an execution profile pins names a primary model; the secondary
/// answers with its own configured model instead.
pub(super) fn unpinned(thread: &Thread) -> Cow<'_, Thread> {
    if thread.profile().model().is_none() {
        return Cow::Borrowed(thread);
    }
    Cow::Owned(thread.clone().without_model())
}

pub(super) fn failover_code(response: &ModelResponse) -> Option<&str> {
    match &response.stop {
        StopReason::Error { code, .. } if FAILOVER_CODES.contains(&code.as_str()) => Some(code),
        _ => None,
    }
}

/// The name a response carries, for the failover log line.
pub(super) fn name(response: &ModelResponse) -> &str {
    response.model.as_deref().unwrap_or_default()
}

pub(super) struct Discard;

#[expect(
    clippy::unused_async_trait_impl,
    reason = "Preserve execution on poll across the async trait seam."
)]
impl Sink for Discard {
    type Error = Infallible;

    async fn emit(&mut self, _event: Event) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Failure codes another provider can plausibly answer. They arrive only after
/// the primary's own retries are spent; every other code goes up unchanged.
const FAILOVER_CODES: [&str; 8] = [
    "provider_unavailable",
    "rate_limited",
    "request_timeout",
    "transport_error",
    agent::CapabilityError::Tools.code(),
    agent::CapabilityError::Images.code(),
    agent::CapabilityError::ToolImages.code(),
    agent::CapabilityError::ReasoningEffort.code(),
];
