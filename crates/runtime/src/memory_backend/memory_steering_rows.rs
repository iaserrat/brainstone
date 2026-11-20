use std::collections::HashMap;

use crate::memory_backend::memory_state::RequestKey;
use crate::memory_backend::memory_steering::{Row, Steerings};
use agent::Message;

use crate::{Admitted, Error, Operation, OperationId, Prompt};

impl Steerings {
    pub(crate) fn existing(
        &self,
        key: &RequestKey,
        prompt: &Prompt,
        operations: &HashMap<OperationId, Operation>,
    ) -> Result<Option<Admitted>, Error> {
        let Some(row) = self.rows.get(key) else {
            return Ok(None);
        };
        if !row.prompt.same_request(prompt) {
            return Err(Error::IdempotencyConflict);
        }
        Ok(Some(Admitted::Steering {
            operation: operations[&row.operation_id].clone(),
            duplicate: true,
        }))
    }

    pub(crate) fn insert(&mut self, key: RequestKey, prompt: Prompt, operation_id: OperationId) {
        self.order.push(key.clone());
        self.rows.insert(
            key,
            Row {
                prompt,
                operation_id,
                consumed: false,
            },
        );
    }

    pub(crate) fn pending(&self, operation_id: OperationId) -> Vec<(RequestKey, Message)> {
        self.order
            .iter()
            .filter_map(|key| {
                let row = &self.rows[key];
                (row.operation_id == operation_id && !row.consumed)
                    .then(|| (key.clone(), row.prompt.message()))
            })
            .take(1)
            .collect()
    }

    pub(crate) fn consume(&mut self, pending: &[(RequestKey, Message)]) {
        for (key, _) in pending {
            self.rows
                .get_mut(key)
                .expect("pending steering exists")
                .consumed = true;
        }
    }
}
