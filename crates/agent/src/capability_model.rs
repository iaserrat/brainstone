use crate::{Model, ModelCapabilities, ModelResponse, Sink, Thread};
use serde_json::Value;

/// Enforce a target's capabilities before I/O without weakening the model seam.
#[derive(Clone, Debug)]
pub struct CapabilityModel<M> {
    model: M,
    capabilities: ModelCapabilities,
    context_window: Option<usize>,
}

impl<M> CapabilityModel<M> {
    #[must_use]
    /// Wraps `model`, refusing requests `capabilities` cannot honor.
    pub const fn new(model: M, capabilities: ModelCapabilities) -> Self {
        Self {
            model,
            capabilities,
            context_window: None,
        }
    }

    /// Context size supplied by the operator or model catalog.
    #[must_use]
    pub const fn with_context_window(mut self, tokens: std::num::NonZeroUsize) -> Self {
        self.context_window = Some(tokens.get());
        self
    }
}

impl<M: Model> Model for CapabilityModel<M> {
    fn validate_request(
        &self,
        thread: &Thread,
        tools: &[Value],
    ) -> Result<(), crate::CapabilityError> {
        self.capabilities.validate(thread, !tools.is_empty())?;
        self.model.validate_request(thread, tools)
    }

    fn context_window(&self) -> Option<usize> {
        self.context_window.or_else(|| self.model.context_window())
    }

    async fn generate(&self, thread: &Thread, tools: &[Value]) -> ModelResponse {
        if let Err(error) = self.validate_request(thread, tools) {
            return error.response();
        }
        self.model.generate(thread, tools).await
    }

    async fn generate_stream<S: Sink + Send>(
        &self,
        thread: &Thread,
        tools: &[Value],
        sink: &mut S,
    ) -> Result<ModelResponse, S::Error> {
        if let Err(error) = self.validate_request(thread, tools) {
            return Ok(error.response());
        }
        if !self.capabilities.streaming {
            return Ok(self.model.generate(thread, tools).await);
        }
        self.model.generate_stream(thread, tools, sink).await
    }

    async fn output_feedback(&self, thread: &Thread) -> Option<String> {
        self.model.output_feedback(thread).await
    }
}
