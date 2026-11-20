use agent::{Message, Model, Step};

use super::Execution;
use super::compaction::{Compacted, exceeded};
use crate::{Error, Settled, Store};

impl<S: Store, M: Model> Execution<'_, S, M> {
    pub(super) async fn step(&mut self, step: Step) -> Result<bool, Error> {
        match step {
            Step::Done(outcome) => self.finish_output(&outcome).await,
            Step::CallModel => self.call_next().await,
            Step::RunTools(calls) => {
                self.call_tools(&calls).await?;
                Ok(false)
            }
            Step::FailTruncated(calls) => {
                let results = calls.iter().map(agent::refused).collect();
                self.append(Message::ToolResults(results)).await?;
                Ok(false)
            }
            // A step this build was not written for cannot be executed safely.
            _ => Err(Error::corrupt("unknown step")),
        }
    }

    async fn finish_output(&mut self, outcome: &agent::Outcome) -> Result<bool, Error> {
        if let Some(feedback) = agent::output_feedback(self.model, &self.claimed.thread).await {
            self.append(Message::OutputCorrection(feedback)).await?;
            return Ok(false);
        }
        self.settle(outcome).await
    }

    async fn call_next(&mut self) -> Result<bool, Error> {
        if matches!(self.claimed.thread.last(), Message::OutputCorrection(_)) {
            return self.answer().await;
        }
        if !self.claimed.thread.last().is_user_turn() {
            self.apply_steering().await?;
        }
        match self.compact().await? {
            Compacted::NotNeeded => self.answer().await,
            Compacted::Done => Ok(false),
            // Fail closed. Proceeding at reduced context would need a durable
            // boundary the journal has no event for, so the turn ends instead.
            Compacted::Failed(outcome) => self.settle(&outcome).await,
        }
    }

    /// One regular model call. The estimate can undercount, so a request the
    /// provider refuses as too long compacts the thread and the loop asks
    /// again; with nothing to compact, the refusal ends the turn as usual.
    async fn answer(&mut self) -> Result<bool, Error> {
        let Some(response) = self.call_model().await? else {
            return Ok(false);
        };
        if exceeded(&response) {
            match self.compact_refused().await? {
                Compacted::NotNeeded => {}
                Compacted::Done => return Ok(false),
                Compacted::Failed(outcome) => return self.settle(&outcome).await,
            }
        }
        self.append(Message::Assistant(response)).await?;
        Ok(false)
    }

    pub(super) async fn settle(&mut self, outcome: &agent::Outcome) -> Result<bool, Error> {
        match self.store.settle(&self.claimed.lease, outcome).await? {
            Settled::Finished => Ok(true),
            Settled::Steered(messages) => {
                self.push(messages);
                Ok(false)
            }
        }
    }
}
