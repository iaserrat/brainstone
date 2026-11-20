use crate::sqlite_backend::{
    sqlite_accept, sqlite_admit, sqlite_claim, sqlite_claim_target, sqlite_compaction,
    sqlite_lease, sqlite_media, sqlite_read, sqlite_record, sqlite_steering,
};
use std::time::Duration;

use sqlx::SqlitePool;

use agent::Event;
use serde_json::Value;

use crate::{
    Accepted, Admitted, Claimed, Compaction, Error, Lease, Operation, OperationId, Prompt, Settled,
    Store,
};

/// A SQLite-backed store.
#[derive(Debug, Clone)]
pub struct SqliteStore {
    pub(crate) pool: SqlitePool,
}

impl Store for SqliteStore {
    async fn authorize_tool(
        &self,
        lease: &Lease,
        request: &crate::ApprovalRequest,
    ) -> Result<crate::ApprovalState, Error> {
        crate::DurableStore::Sqlite(self.clone())
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
        sqlite_accept::accept(&self.pool, prompt, definitions).await
    }

    async fn admit_with_tools(
        &self,
        prompt: &Prompt,
        definitions: &[Value],
    ) -> Result<Admitted, Error> {
        sqlite_admit::admit(&self.pool, prompt, definitions).await
    }

    async fn operation(&self, id: OperationId) -> Result<Option<Operation>, Error> {
        sqlite_read::operation(&self.pool, id).await
    }

    async fn claim(&self, worker: &str, ttl: Duration) -> Result<Option<Claimed>, Error> {
        sqlite_claim::claim(&self.pool, worker, ttl).await
    }

    async fn claim_operation(
        &self,
        id: OperationId,
        worker: &str,
        ttl: Duration,
    ) -> Result<Option<Claimed>, Error> {
        sqlite_claim_target::claim(&self.pool, id, worker, ttl).await
    }

    async fn record(&self, lease: &Lease, event: &Event) -> Result<(), Error> {
        sqlite_record::record(&self.pool, lease, event).await
    }

    async fn save_media(&self, lease: &Lease, media: &[crate::Media]) -> Result<(), Error> {
        sqlite_media::save(&self.pool, lease, media).await
    }

    async fn steering(&self, lease: &Lease) -> Result<Vec<agent::Message>, Error> {
        sqlite_steering::drain(&self.pool, lease).await
    }

    async fn has_steering(&self, lease: &Lease) -> Result<Option<bool>, Error> {
        sqlite_steering::has_pending(&self.pool, lease)
            .await
            .map(Some)
    }

    async fn settle(&self, lease: &Lease, outcome: &agent::Outcome) -> Result<Settled, Error> {
        sqlite_steering::settle(&self.pool, lease, outcome).await
    }

    async fn heartbeat(&self, lease: &Lease, ttl: Duration) -> Result<(), Error> {
        sqlite_lease::heartbeat(&self.pool, lease, ttl).await
    }

    async fn compact(&self, compaction: &Compaction) -> Result<(), Error> {
        sqlite_compaction::compact(&self.pool, compaction).await
    }
}
