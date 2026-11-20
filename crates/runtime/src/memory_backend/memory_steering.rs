use agent::{Event, Message, Outcome};

use crate::memory_backend::memory_state::{RequestKey, State, request_key};
use crate::{Admitted, Error, Lease, OperationId, OperationState, Prompt, Settled};

#[derive(Debug, Default)]
pub(crate) struct Steerings {
    pub(crate) rows: std::collections::HashMap<RequestKey, Row>,
    pub(crate) order: Vec<RequestKey>,
}

#[derive(Debug)]
pub(crate) struct Row {
    pub(crate) prompt: Prompt,
    pub(crate) operation_id: OperationId,
    pub(crate) consumed: bool,
}

impl State {
    pub(crate) fn admit(&mut self, prompt: &Prompt) -> Result<Admitted, Error> {
        let prompt = self.addresses.admission_prompt(prompt);
        let prompt = &prompt;
        let key = request_key(prompt);
        if let Some(value) = self.existing(&key, prompt)? {
            return Ok(value.into());
        }
        if let Some(value) = self.steerings.existing(&key, prompt, &self.operations)? {
            return Ok(value);
        }
        let (_, lane_id) = self.addresses.resolve(prompt)?;
        let active = self.active_for(lane_id, prompt);
        let Some(operation) = active.cloned() else {
            return self.accept(prompt, true).map(Into::into);
        };
        self.steerings.insert(key, prompt.clone(), operation.id);
        self.deliveries.invalidate_presentation(operation.id);
        Ok(Admitted::Steering {
            operation,
            duplicate: false,
        })
    }

    fn active_for(&self, lane_id: crate::LaneId, prompt: &Prompt) -> Option<&crate::Operation> {
        self.operations.values().find(|operation| {
            operation.lane_id == lane_id
                && operation.state == OperationState::Running
                && self
                    .runs
                    .prompt(operation.id)
                    .is_some_and(|stored| stored.identity == prompt.identity)
        })
    }

    pub(crate) fn steering(&mut self, lease: &Lease) -> Result<Vec<Message>, Error> {
        let pending = self.steerings.pending(lease.operation_id);
        for (_, message) in &pending {
            self.record(
                lease,
                &Event::Appended {
                    message: message.clone(),
                },
            )?;
        }
        self.steerings.consume(&pending);
        Ok(pending.into_iter().map(|(_, message)| message).collect())
    }

    pub(crate) fn settle(&mut self, lease: &Lease, outcome: &Outcome) -> Result<Settled, Error> {
        let messages = self.steering(lease)?;
        if !messages.is_empty() {
            return Ok(Settled::Steered(messages));
        }
        self.record(
            lease,
            &Event::RunFinished {
                outcome: outcome.clone(),
            },
        )?;
        Ok(Settled::Finished)
    }
}
