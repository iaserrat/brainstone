use crate::durable_backend::durable::dispatch;
use crate::{DurableStore, Interaction, InteractionRequest, InteractionStore};
use serde_json::Value;

impl InteractionStore for DurableStore {
    async fn open_interaction(
        &self,
        request: &InteractionRequest,
    ) -> Result<Interaction, crate::Error> {
        dispatch!(self, open_interaction(request))
    }

    async fn mark_interaction_delivered(&self, id: &str, ts: &str) -> Result<(), crate::Error> {
        dispatch!(self, mark_interaction_delivered(id, ts))
    }

    async fn answer_interaction(&self, id: &str, response: &Value) -> Result<(), crate::Error> {
        dispatch!(self, answer_interaction(id, response))
    }

    async fn interaction(&self, id: &str) -> Result<Option<Interaction>, crate::Error> {
        dispatch!(self, interaction(id))
    }
}
