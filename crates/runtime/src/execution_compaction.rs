use agent::{Event, Model, ModelResponse, Outcome, StopReason};

use super::Execution;
use super::summary::{rejected, usable};
use crate::{CompactionPolicy, Error, Store};

#[path = "execution_summary_attempt.rs"]
mod attempt;

pub(super) enum Compacted {
    /// Under the limit. The regular model call proceeds.
    NotNeeded,
    /// The thread now starts from a durable summary.
    Done,
    /// No usable summary. The turn ends here, not the process: an
    /// unsummarizable thread is one lane's problem, not the replica's.
    Failed(Outcome),
}

impl<S: Store, M: Model> Execution<'_, S, M> {
    pub(super) async fn compact(&mut self) -> Result<Compacted, Error> {
        let Some(policy) = self.compaction else {
            return Ok(Compacted::NotNeeded);
        };
        if !policy.reached_with_window(
            &self.claimed.thread,
            &self.definitions,
            self.model.context_window(),
        )? {
            return Ok(Compacted::NotNeeded);
        }
        self.summarize_thread(policy).await
    }

    /// Compaction the provider asked for by refusing a regular request as too
    /// long. A lone message has nothing to leave out: the smallest rung would
    /// be that same message, so the refusal stands.
    pub(super) async fn compact_refused(&mut self) -> Result<Compacted, Error> {
        match self.compaction {
            Some(policy) if self.claimed.thread.messages().len() > 1 => {
                self.summarize_thread(policy).await
            }
            _ => Ok(Compacted::NotNeeded),
        }
    }

    async fn summarize_thread(&mut self, policy: &CompactionPolicy) -> Result<Compacted, Error> {
        agent::observation::run(
            agent::observation::span(agent::observation::Kind::Compaction),
            self.summarize_inner(policy),
            |result| match result {
                Ok(Compacted::Done | Compacted::NotNeeded) => "ok",
                _ => "failed",
            },
        )
        .await
    }

    async fn summarize_inner(&mut self, policy: &CompactionPolicy) -> Result<Compacted, Error> {
        if let Some(outcome) = &self.summary_failure {
            return Ok(Compacted::Failed(outcome.clone()));
        }
        let response = self.fitted(policy).await;
        let Some(summary) = usable(&response) else {
            return Ok(Compacted::Failed(self.remember(rejected(response))));
        };
        let event = Event::Compacted {
            summary: summary.clone(),
            steps: self.claimed.thread.steps(),
        };
        self.record(&event).await?;
        self.claimed.thread.compact(summary);
        Ok(Compacted::Done)
    }

    fn remember(&mut self, outcome: Outcome) -> Outcome {
        self.summary_failure = Some(outcome.clone());
        outcome
    }

    /// The first settled summary a rung of the ladder fits. A rung refused as
    /// too long hands on to the next; any other answer is final.
    async fn fitted(&self, policy: &CompactionPolicy) -> ModelResponse {
        let mut response = None;
        for request in policy.summary_requests(&self.claimed.thread) {
            let answer = self.summarize(&request).await;
            let overflowed = exceeded(&answer);
            response = Some(answer);
            if !overflowed {
                break;
            }
        }
        response.expect("a ladder has at least one rung")
    }
}

pub(super) fn exceeded(response: &ModelResponse) -> bool {
    matches!(&response.stop, StopReason::Error { code, .. }
        if code == agent::CODE_CONTEXT_WINDOW_EXCEEDED)
}
