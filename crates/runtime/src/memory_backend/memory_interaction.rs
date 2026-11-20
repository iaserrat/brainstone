use serde_json::Value;

use crate::memory_backend::memory_state::State;
use crate::{Error, Interaction, InteractionRequest};

impl State {
    pub(crate) fn open_interaction(
        &mut self,
        request: &InteractionRequest,
    ) -> Result<Interaction, Error> {
        if let Some(stored) = self.interactions.get(&request.id) {
            return (stored.request == *request)
                .then(|| stored.clone())
                .ok_or(Error::IdempotencyConflict);
        }
        let prompt = self.runs.prompt(request.operation_id).ok_or_else(missing)?;
        let target = prompt.delivery.clone().ok_or(Error::InvalidInput {
            field: "interaction_delivery",
        })?;
        let interaction = Interaction {
            request: request.clone(),
            target,
            client_message_id: format!("interaction-{}", request.id),
            external_id: None,
            response: None,
        };
        self.interactions
            .insert(request.id.clone(), interaction.clone());
        Ok(interaction)
    }

    pub(crate) fn interaction(&self, id: &str) -> Option<Interaction> {
        self.interactions.get(id).cloned()
    }

    pub(crate) fn deliver_interaction(&mut self, id: &str, ts: &str) -> Result<(), Error> {
        set_once(
            self.interactions.get_mut(id),
            |row| &mut row.external_id,
            ts.to_owned(),
        )
    }

    pub(crate) fn answer_interaction(&mut self, id: &str, value: &Value) -> Result<(), Error> {
        set_once(
            self.interactions.get_mut(id),
            |row| &mut row.response,
            value.clone(),
        )
    }
}

fn set_once<T: PartialEq>(
    row: Option<&mut Interaction>,
    field: impl FnOnce(&mut Interaction) -> &mut Option<T>,
    value: T,
) -> Result<(), Error> {
    let row = row.ok_or(Error::IdempotencyConflict)?;
    let slot = field(row);
    if slot.as_ref().is_some_and(|stored| stored != &value) {
        return Err(Error::IdempotencyConflict);
    }
    *slot = Some(value);
    Ok(())
}

fn missing() -> Error {
    Error::InvalidInput {
        field: "operation_id",
    }
}
