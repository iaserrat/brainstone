use agent::{Event, Model, ModelResponse, Sink, Thread};
use reqwest::Client;
use serde_json::Value;
use std::fmt;

use crate::{Auth, ConfigError, Effort, Endpoint};

#[derive(Clone)]
/// The Anthropic Messages adapter over SSE endpoints or Amazon Bedrock.
pub struct AnthropicModel {
    pub(crate) client: Client,
    pub(crate) endpoint: Endpoint,
    pub(crate) model: String,
    pub(crate) auth: Auth,
    pub(crate) max_tokens: u32,
    pub(crate) effort: Option<Effort>,
    pub(crate) prompt_cache: bool,
}

impl AnthropicModel {
    /// # Errors
    ///
    /// Returns a stable configuration error for invalid input.
    pub fn new(
        endpoint: Endpoint,
        model: &str,
        auth: Auth,
        max_tokens: u32,
    ) -> Result<Self, ConfigError> {
        crate::model_build::build(endpoint, model, auth, max_tokens)
    }

    #[must_use]
    /// Sets the reasoning effort the request asks for.
    pub const fn with_effort(mut self, effort: Effort) -> Self {
        self.effort = Some(effort);
        self
    }

    /// Ask for automatic prompt caching: a 5-minute breakpoint the API
    /// places on the last cacheable block and moves forward each turn.
    #[must_use]
    pub const fn with_prompt_cache(mut self) -> Self {
        self.prompt_cache = true;
        self
    }

    /// Hand every request to one HTTP proxy instead of the host's own route.
    ///
    /// # Errors
    /// Reports an unusable proxy URL or HTTP client configuration failure.
    pub fn with_egress(mut self, egress: &agent::Egress) -> Result<Self, ConfigError> {
        self.client = crate::model_build::client(egress)?;
        Ok(self)
    }
}

impl Model for AnthropicModel {
    async fn generate(&self, thread: &Thread, tools: &[Value]) -> ModelResponse {
        crate::stream::generate(self, thread, tools, &mut Noop)
            .await
            .unwrap_or_else(|never| match never {})
    }

    async fn generate_stream<S: Sink + Send>(
        &self,
        thread: &Thread,
        tools: &[Value],
        sink: &mut S,
    ) -> Result<ModelResponse, S::Error> {
        crate::stream::generate(self, thread, tools, sink).await
    }
}

struct Noop;

#[expect(
    clippy::unused_async_trait_impl,
    reason = "Preserve execution on poll across the async trait seam."
)]
impl Sink for Noop {
    type Error = std::convert::Infallible;

    async fn emit(&mut self, _event: Event) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl fmt::Debug for AnthropicModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnthropicModel")
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .field("auth", &self.auth)
            .field("max_tokens", &self.max_tokens)
            .field("effort", &self.effort)
            .field("prompt_cache", &self.prompt_cache)
            .finish_non_exhaustive()
    }
}
