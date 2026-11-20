use agent::{Model, Registry};

use super::Runtime;
use crate::{Error, Operation, OperationId, Store};

#[path = "runtime_claim.rs"]
mod claim;

impl<S: Store> Runtime<S> {
    /// Run at most one pending operation to a durable outcome.
    ///
    /// # Errors
    /// Returns a stable runtime error when durable execution fails.
    pub async fn run_next<M: Model>(
        &self,
        model: &M,
        tools: &Registry,
    ) -> Result<Option<Operation>, Error> {
        let Some(claimed) = self
            .store
            .claim(&self.config.worker, self.config.lease_ttl)
            .await?
        else {
            return Ok(None);
        };
        self.run_claim(claimed, model, tools).await
    }

    /// Run one named pending operation when its session is available.
    ///
    /// # Errors
    /// Returns a stable runtime error when durable execution fails.
    pub async fn run_operation<M: Model>(
        &self,
        id: OperationId,
        model: &M,
        tools: &Registry,
    ) -> Result<Option<Operation>, Error> {
        let Some(claimed) = self
            .store
            .claim_operation(id, &self.config.worker, self.config.lease_ttl)
            .await?
        else {
            return Ok(None);
        };
        self.run_claim(claimed, model, tools).await
    }
}
