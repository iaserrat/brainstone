use crate::postgres_backend::{
    postgres_accept, postgres_admit, postgres_claim, postgres_claim_target, postgres_compaction,
    postgres_lease, postgres_media, postgres_read, postgres_record, postgres_steering,
};
use std::time::Duration;

use sqlx::PgPool;

use agent::Event;
use serde_json::Value;

use crate::{
    Accepted, Admitted, Claimed, Compaction, Error, Lease, Operation, OperationId, Prompt, Settled,
    Store,
};

/// A PostgreSQL-backed store.
#[derive(Debug, Clone)]
pub struct PostgresStore {
    pub(crate) pool: PgPool,
}

impl Store for PostgresStore {
    async fn authorize_tool(
        &self,
        lease: &Lease,
        request: &crate::ApprovalRequest,
    ) -> Result<crate::ApprovalState, Error> {
        crate::DurableStore::Postgres(self.clone())
            .authorize(lease, request)
            .await
    }
    async fn accept(&self, prompt: &Prompt) -> Result<Accepted, Error> {
        self.accept_with_tools(prompt, &[]).await
    }

    async fn accept_with_tools(
        &self,
        prompt: &Prompt,
        definitions: &[Value],
    ) -> Result<Accepted, Error> {
        postgres_accept::accept(&self.pool, prompt, definitions).await
    }

    async fn admit_with_tools(
        &self,
        prompt: &Prompt,
        definitions: &[Value],
    ) -> Result<Admitted, Error> {
        postgres_admit::admit(&self.pool, prompt, definitions).await
    }

    async fn operation(&self, id: OperationId) -> Result<Option<Operation>, Error> {
        postgres_read::operation(&self.pool, id).await
    }

    async fn claim(&self, worker: &str, ttl: Duration) -> Result<Option<Claimed>, Error> {
        postgres_claim::claim(&self.pool, worker, ttl).await
    }

    async fn claim_operation(
        &self,
        id: OperationId,
        worker: &str,
        ttl: Duration,
    ) -> Result<Option<Claimed>, Error> {
        postgres_claim_target::claim(&self.pool, id, worker, ttl).await
    }

    async fn record(&self, lease: &Lease, event: &Event) -> Result<(), Error> {
        postgres_record::record(&self.pool, lease, event).await
    }

    async fn save_media(&self, lease: &Lease, media: &[crate::Media]) -> Result<(), Error> {
        postgres_media::save(&self.pool, lease, media).await
    }

    async fn steering(&self, lease: &Lease) -> Result<Vec<agent::Message>, Error> {
        postgres_steering::drain(&self.pool, lease).await
    }

    async fn has_steering(&self, lease: &Lease) -> Result<Option<bool>, Error> {
        postgres_steering::has_pending(&self.pool, lease)
            .await
            .map(Some)
    }

    async fn settle(&self, lease: &Lease, outcome: &agent::Outcome) -> Result<Settled, Error> {
        postgres_steering::settle(&self.pool, lease, outcome).await
    }

    async fn heartbeat(&self, lease: &Lease, ttl: Duration) -> Result<(), Error> {
        postgres_lease::heartbeat(&self.pool, lease, ttl).await
    }

    async fn compact(&self, compaction: &Compaction) -> Result<(), Error> {
        postgres_compaction::compact(&self.pool, compaction).await
    }
}
