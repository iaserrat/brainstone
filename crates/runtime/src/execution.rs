use agent::{Event, Message, Model, Registry};
use serde_json::Value;

use crate::drive::Effects;
use crate::{Claimed, CompactionPolicy, Error, Store};

#[path = "execution_compaction.rs"]
pub(crate) mod compaction;
#[path = "execution_step.rs"]
mod step;
#[path = "execution_summary.rs"]
pub(crate) mod summary;

#[path = "execution_approval.rs"]
mod approval;
#[path = "execution_contract.rs"]
mod contract;
#[path = "execution_model.rs"]
mod model;
#[path = "execution_steering.rs"]
pub(crate) mod steering;
#[path = "execution_tool.rs"]
mod tool;

pub(crate) struct Execution<'a, S, M> {
    store: &'a S,
    claimed: &'a mut Claimed,
    model: &'a M,
    /// The tools this claim offers, fixed for the claim so every request in
    /// the loop shares one cacheable prefix.
    tools: Registry,
    definitions: Vec<Value>,
    compaction: Option<&'a CompactionPolicy>,
    policy: Option<&'a crate::ToolPolicy>,
    /// Set once summarizing has failed here: absorbed steering sends the loop
    /// round again, and one more message will not summarize any better.
    summary_failure: Option<agent::Outcome>,
}

impl<'a, S: Store, M: Model> Execution<'a, S, M> {
    pub(crate) fn new(
        store: &'a S,
        claimed: &'a mut Claimed,
        effects: &Effects<'a, M>,
        tools: Registry,
    ) -> Self {
        let tools = match claimed.thread.profile().tool_contract() {
            Some(contract) => tools.select(contract.allowed_tools(), &[]),
            None => tools,
        };
        Self {
            store,
            claimed,
            model: effects.model,
            definitions: tools.definitions(),
            tools,
            compaction: effects.compaction,
            policy: effects.policy,
            summary_failure: None,
        }
    }

    pub(crate) async fn run(mut self) -> Result<(), Error> {
        self.record(&Event::RunStarted).await?;
        if let Some(outcome) = self.unavailable_tools() {
            self.settle_contract(&outcome).await?;
            return Ok(());
        }
        if !matches!(
            agent::next(&self.claimed.thread),
            agent::Step::RunTools(_) | agent::Step::FailTruncated(_)
        ) {
            self.apply_steering().await?;
        }
        loop {
            let step = agent::next(&self.claimed.thread);
            if self.step(step).await? {
                return Ok(());
            }
        }
    }

    async fn apply_steering(&mut self) -> Result<bool, Error> {
        if self.store.has_steering(&self.claimed.lease).await? == Some(false) {
            return Ok(false);
        }
        let messages = self.store.steering(&self.claimed.lease).await?;
        let changed = !messages.is_empty();
        self.push(messages);
        Ok(changed)
    }

    async fn append(&mut self, message: Message) -> Result<(), Error> {
        self.record(&Event::Appended {
            message: message.clone(),
        })
        .await?;
        self.claimed.thread.push(message);
        Ok(())
    }

    async fn record(&self, event: &Event) -> Result<(), Error> {
        self.store.record(&self.claimed.lease, event).await
    }
}
