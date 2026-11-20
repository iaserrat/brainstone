use crate::durable_backend::durable::dispatch;
use crate::{
    Accepted, Admitted, Claimed, Compaction, DurableStore, Error, Lease, Operation, OperationId,
    Prompt, Settled, Store,
};
use agent::Event;
use serde_json::Value;
use std::time::Duration;

impl Store for DurableStore {
    async fn authorize_tool(
        &self,
        lease: &Lease,
        request: &crate::ApprovalRequest,
    ) -> Result<crate::ApprovalState, Error> {
        self.authorize(lease, request).await
    }
    async fn accept(&self, prompt: &Prompt) -> Result<Accepted, Error> {
        dispatch!(self, accept(prompt))
    }

    async fn accept_with_tools(
        &self,
        prompt: &Prompt,
        definitions: &[Value],
    ) -> Result<Accepted, Error> {
        dispatch!(self, accept_with_tools(prompt, definitions))
    }

    async fn admit_with_tools(
        &self,
        prompt: &Prompt,
        definitions: &[Value],
    ) -> Result<Admitted, Error> {
        dispatch!(self, admit_with_tools(prompt, definitions))
    }

    async fn operation(&self, id: OperationId) -> Result<Option<Operation>, Error> {
        dispatch!(self, operation(id))
    }

    async fn claim(&self, worker: &str, ttl: Duration) -> Result<Option<Claimed>, Error> {
        dispatch!(self, claim(worker, ttl))
    }

    async fn claim_operation(
        &self,
        id: OperationId,
        worker: &str,
        ttl: Duration,
    ) -> Result<Option<Claimed>, Error> {
        dispatch!(self, claim_operation(id, worker, ttl))
    }

    async fn record(&self, lease: &Lease, event: &Event) -> Result<(), Error> {
        dispatch!(self, record(lease, event))
    }

    async fn save_media(&self, lease: &Lease, media: &[crate::Media]) -> Result<(), Error> {
        dispatch!(self, save_media(lease, media))
    }

    async fn steering(&self, lease: &Lease) -> Result<Vec<agent::Message>, Error> {
        dispatch!(self, steering(lease))
    }

    async fn has_steering(&self, lease: &Lease) -> Result<Option<bool>, Error> {
        dispatch!(self, has_steering(lease))
    }

    async fn settle(&self, lease: &Lease, outcome: &agent::Outcome) -> Result<Settled, Error> {
        dispatch!(self, settle(lease, outcome))
    }

    async fn heartbeat(&self, lease: &Lease, ttl: Duration) -> Result<(), Error> {
        dispatch!(self, heartbeat(lease, ttl))
    }

    async fn compact(&self, compaction: &Compaction) -> Result<(), Error> {
        dispatch!(self, compact(compaction))
    }
}
