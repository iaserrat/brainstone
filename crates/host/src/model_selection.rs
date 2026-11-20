//! Operator-configured private-stdio choice of each prompt's model and
//! reasoning effort, made before durable acceptance so recovery replays it.
use runtime::{LaneStore, OperationId, Prompt};

use crate::{Admission, Error, ModelSelector};

/// Sets each new prompt's model and effort from the selector. Without one,
/// prompts pass through untouched. A redelivered request keeps the profile
/// its first admission stored, so the selector is not asked again.
#[derive(Clone, Debug)]
pub struct ModelSelection<A, S> {
    inner: A,
    store: S,
    selector: Option<ModelSelector>,
}

impl<A, S> ModelSelection<A, S> {
    #[must_use]
    /// Wraps `inner`, asking `selector` for each new prompt's model and effort.
    pub const fn new(inner: A, store: S, selector: Option<ModelSelector>) -> Self {
        Self {
            inner,
            store,
            selector,
        }
    }
}

impl<A: Admission, S: LaneStore + Clone + Send + Sync> Admission for ModelSelection<A, S> {
    async fn preflight(&self, actor: Option<&str>) -> Result<(), Error> {
        self.inner.preflight(actor).await
    }

    async fn accept(&self, prompt: &Prompt) -> Result<OperationId, Error> {
        let Some(selector) = &self.selector else {
            return self.inner.accept(prompt).await;
        };
        if self
            .store
            .request_exists(prompt.address(), prompt.request_id())
            .await?
        {
            return self.inner.accept(prompt).await;
        }
        let profile = selector.select(prompt).await?;
        self.inner
            .accept(&prompt.clone().with_profile(profile))
            .await
    }
}
