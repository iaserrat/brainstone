use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::{Accepted, Admitted, Compaction, Config, Error, Operation, OperationId, Prompt, Store};
use agent::Registry;

#[path = "runtime_run.rs"]
mod run;
#[path = "runtime_subagent.rs"]
mod subagent;

const LEASE_TTL: Duration = Duration::from_secs(30);
static WORKER_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Accepts and runs durable operations over a store.
#[derive(Debug, Clone)]
pub struct Runtime<S> {
    store: S,
    config: Config,
}

impl<S: Store> Runtime<S> {
    #[must_use]
    /// A runtime with the default configuration.
    pub fn new(store: S) -> Self {
        let config = Config::automatic(worker_id(), LEASE_TTL);
        Self { store, config }
    }

    #[must_use]
    /// A runtime with explicit tunables.
    pub const fn with_config(store: S, config: Config) -> Self {
        Self { store, config }
    }

    /// # Errors
    ///
    /// Returns a stable runtime error when the operation cannot be committed.
    pub async fn accept(&self, prompt: &Prompt) -> Result<Accepted, Error> {
        self.store.accept(prompt).await
    }

    /// Atomically accept a prompt and pin this lane's model-facing tool catalog.
    ///
    /// # Errors
    ///
    /// Returns a stable runtime error when the operation cannot be committed.
    pub async fn accept_with_tools(
        &self,
        prompt: &Prompt,
        tools: &Registry,
    ) -> Result<Accepted, Error> {
        self.store
            .accept_with_tools(prompt, &tools.definitions())
            .await
    }

    /// Durably steer the active lane, or accept a new operation when idle.
    /// The supplied system prompt creates new lanes; existing lanes keep their
    /// stored prompt across deployment configuration changes.
    ///
    /// # Errors
    /// Returns a stable persistence or idempotency error.
    pub async fn admit_with_tools(
        &self,
        prompt: &Prompt,
        tools: &Registry,
    ) -> Result<Admitted, Error> {
        self.store
            .admit_with_tools(prompt, &tools.definitions())
            .await
    }

    /// # Errors
    ///
    /// Returns a stable runtime error when the operation cannot be read.
    pub async fn operation(&self, id: OperationId) -> Result<Option<Operation>, Error> {
        self.store.operation(id).await
    }

    /// Replace a completed lane prefix with one durable summary.
    ///
    /// No surface in this workspace calls this, so a lane wedged by a failed
    /// automatic compaction has no operator recovery path today: it is reachable
    /// only by an embedder that wires it up itself.
    ///
    /// # Errors
    ///
    /// Returns `invalid_compaction` for an empty or unsafe boundary.
    pub async fn compact(&self, compaction: &Compaction) -> Result<(), Error> {
        if compaction.summary.is_empty() {
            return Err(Error::InvalidCompaction);
        }
        self.store.compact(compaction).await
    }
}

fn worker_id() -> String {
    let sequence = WORKER_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{}-{sequence}", std::process::id())
}
