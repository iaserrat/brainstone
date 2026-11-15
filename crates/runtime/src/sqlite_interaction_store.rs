use serde_json::Value;

use crate::{Interaction, InteractionRequest, InteractionStore, SqliteStore};

impl InteractionStore for SqliteStore {
    async fn open_interaction(
        &self,
        request: &InteractionRequest,
    ) -> Result<Interaction, crate::Error> {
        crate::sqlite_interaction::open(&self.pool, request).await
    }

    async fn mark_interaction_delivered(&self, id: &str, ts: &str) -> Result<(), crate::Error> {
        crate::sqlite_interaction::mark_delivered(&self.pool, id, ts).await
    }

    async fn answer_interaction(&self, id: &str, response: &Value) -> Result<(), crate::Error> {
        crate::sqlite_interaction::answer(&self.pool, id, response).await
    }

    async fn interaction(&self, id: &str) -> Result<Option<Interaction>, crate::Error> {
        crate::sqlite_interaction::read(&self.pool, id).await
    }
}
