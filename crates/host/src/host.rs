use std::sync::Arc;

use agent::{Model, Registry};
use runtime::{Accepted, Admitted, Prompt, Runtime, Store};
use tokio::sync::{Notify, RwLock};

use crate::{Config, Error};

/// Claims accepted operations, runs them, and records their outcomes.
pub struct Host<S, M> {
    pub(crate) inner: Arc<Inner<S, M>>,
}

pub(crate) struct Inner<S, M> {
    pub(crate) runtime: Runtime<S>,
    pub(crate) model: M,
    pub(crate) tools: Registry,
    pub(crate) config: Config,
    pub(crate) accepting: RwLock<bool>,
    pub(crate) wake: Notify,
    /// Fired only by shutdown, so accepted prompts cannot cut a retry pause short.
    pub(crate) closed: Notify,
}

impl<S, M> std::fmt::Debug for Host<S, M> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Host")
            .field("config", &self.inner.config)
            .finish_non_exhaustive()
    }
}

impl<S, M> Clone for Host<S, M> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<S: Store, M: Model> Host<S, M> {
    #[must_use]
    /// A host with the default poll cadence and worker count.
    pub fn new(runtime: Runtime<S>, model: M, tools: Registry) -> Self {
        Self::with_config(runtime, model, tools, Config::default())
    }

    #[must_use]
    /// A host with explicit poll cadence and worker bounds.
    pub fn with_config(runtime: Runtime<S>, model: M, tools: Registry, config: Config) -> Self {
        Self {
            inner: Arc::new(Inner {
                runtime,
                model,
                tools,
                config,
                accepting: RwLock::new(true),
                wake: Notify::new(),
                closed: Notify::new(),
            }),
        }
    }

    /// Accept a prompt before acknowledging its surface.
    ///
    /// # Errors
    ///
    /// Returns `host_shutting_down` after admission closes, or a runtime code
    /// when durable acceptance fails.
    pub async fn accept(&self, prompt: &Prompt) -> Result<Accepted, Error> {
        let accepting = self.inner.accepting.read().await;
        if !*accepting {
            return Err(Error::ShuttingDown);
        }
        let accepted = self
            .inner
            .runtime
            .accept_with_tools(prompt, &self.inner.tools)
            .await?;
        self.inner.wake.notify_one();
        Ok(accepted)
    }

    /// Durably steer an active lane, or accept a new operation when idle.
    ///
    /// # Errors
    /// Returns `host_shutting_down` after admission closes, or a runtime code.
    pub async fn admit(&self, prompt: &Prompt) -> Result<Admitted, Error> {
        let accepting = self.inner.accepting.read().await;
        if !*accepting {
            return Err(Error::ShuttingDown);
        }
        let admitted = self
            .inner
            .runtime
            .admit_with_tools(prompt, &self.inner.tools)
            .await?;
        self.inner.wake.notify_one();
        Ok(admitted)
    }

    /// Stop admission and wait for the current accept or operation boundary.
    pub async fn shutdown(&self) {
        let mut accepting = self.inner.accepting.write().await;
        *accepting = false;
        drop(accepting);
        self.inner.wake.notify_waiters();
        self.inner.closed.notify_waiters();
    }
}
