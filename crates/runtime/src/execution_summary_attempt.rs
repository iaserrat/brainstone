//! One settled summary per request: rewritten at less reasoning if its thinking
//! ate the response budget, retried only if its stream broke.

use std::time::Duration;

use agent::{Effort, Model, ModelResponse, StopReason, Thread};

use crate::Store;
use crate::execution::Execution;
use crate::execution::summary::transient;

/// What a summary is rewritten at once its own thinking has eaten the response
/// budget. Not the first choice: a summary is durable and permanently replaces
/// the prefix, so writing every one at less reasoning than the lane asked for
/// would trade a crash for a quiet, unmeasured downgrade of every later turn.
const SUMMARY_EFFORT: Effort = Effort::Low;

/// Summarizing is worth retrying where a regular turn is not: it runs on a
/// thread already at the limit, so a lost attempt costs the turn *and* leaves
/// the lane too large for the next one. The provider seam owns retries, but
/// only until the response stream opens: a failure raised mid-stream arrives
/// here having been tried exactly once. That gap, not throttling, is what this
/// budget covers — the wait below is sized for a reconnect, and a throttle
/// needs seconds, so `TRANSIENT_CODES` excludes it. Retrying a mid-stream
/// throttle belongs at the provider seam, where `retry-after` is visible.
const MAX_SUMMARY_ATTEMPTS: u32 = 3;
const SUMMARY_RETRY_BACKOFF: Duration = Duration::from_millis(250);

impl<S: Store, M: Model> Execution<'_, S, M> {
    /// One settled summary at the lane's own effort, rewritten at
    /// [`SUMMARY_EFFORT`] only if that overflowed the response budget.
    /// Thinking and the summary share that budget, so less reasoning is what
    /// leaves room for the summary — but only an observed overflow is evidence
    /// the trade is needed, and the rewrite is safe to attempt because a failed
    /// summary now costs the turn rather than the process.
    pub(super) async fn summarize(&self, request: &Thread) -> ModelResponse {
        let response = self.attempt(request).await;
        if !matches!(response.stop, StopReason::MaxTokens) {
            return response;
        }
        let rewritten = request.clone().with_effort(SUMMARY_EFFORT);
        if self.model.validate_request(&rewritten, &[]).is_err() {
            return response;
        }
        self.attempt(&rewritten).await
    }

    /// One settled response, retrying only a stream that broke. An answered
    /// request that came back empty or carrying tool calls would answer the
    /// same way again, so retrying it only spends the budget it exhausted.
    async fn attempt(&self, request: &Thread) -> ModelResponse {
        let mut attempt = 1;
        loop {
            let response = agent::observation::generate(self.model, request, &[]).await;
            if !transient(&response) || attempt == MAX_SUMMARY_ATTEMPTS {
                return response;
            }
            tokio::time::sleep(SUMMARY_RETRY_BACKOFF * attempt).await;
            attempt += 1;
        }
    }
}
