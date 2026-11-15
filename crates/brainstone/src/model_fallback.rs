//! Route between named targets that can honor the request.
//! Each target owns replay and reasoning isolation.

use serde_json::Value;

use agent::{Model, ModelResponse, Sink, Thread};

#[path = "model_fallback_view.rs"]
mod view;
use view::{Discard, failover_code, name, unpinned};

#[derive(Clone, Debug)]
pub struct Fallback<P, S> {
    primary: P,
    secondary: Option<S>,
}

impl<P, S> Fallback<P, S> {
    /// `primary` alone. Requests and responses pass through unchanged.
    #[must_use]
    pub const fn single(primary: P) -> Self {
        Self {
            primary,
            secondary: None,
        }
    }

    /// `primary`, failing over to `secondary`.
    #[must_use]
    pub const fn new(primary: P, secondary: S) -> Self {
        Self {
            primary,
            secondary: Some(secondary),
        }
    }
}

impl<P: Model, S: Model> Model for Fallback<P, S> {
    fn validate_request(
        &self,
        thread: &Thread,
        tools: &[Value],
    ) -> Result<(), agent::CapabilityError> {
        let primary = self.primary.validate_request(thread, tools);
        if primary.is_ok() {
            return primary;
        }
        match &self.secondary {
            Some(secondary) => secondary.validate_request(&unpinned(thread), tools),
            None => primary,
        }
    }

    fn context_window(&self) -> Option<usize> {
        self.primary
            .context_window()
            .into_iter()
            .chain(self.secondary.as_ref().and_then(Model::context_window))
            .min()
    }

    async fn generate(&self, thread: &Thread, tools: &[Value]) -> ModelResponse {
        if self.secondary.is_none() {
            return self.primary.generate(thread, tools).await;
        }
        let Ok(response) = self.generate_stream(thread, tools, &mut Discard).await;
        response
    }

    async fn output_feedback(&self, thread: &Thread) -> Option<String> {
        self.primary.output_feedback(thread).await
    }

    async fn generate_stream<K: Sink + Send>(
        &self,
        thread: &Thread,
        tools: &[Value],
        sink: &mut K,
    ) -> Result<ModelResponse, K::Error> {
        let Some(secondary) = &self.secondary else {
            return self.primary.generate_stream(thread, tools, sink).await;
        };
        let failed = self.primary.generate_stream(thread, tools, sink).await?;
        let Some(code) = failover_code(&failed) else {
            return Ok(failed);
        };
        let shown = unpinned(thread);
        if secondary.validate_request(&shown, tools).is_err() {
            return Ok(failed);
        }
        let response = secondary.generate_stream(&shown, tools, sink).await?;
        tracing::warn!(name: "brainstone.model.failover",
            primary = name(&failed),
            primary_code = code,
            model = name(&response),
            "model failover"
        );
        Ok(response)
    }
}
