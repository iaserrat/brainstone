use crate::{ConfigError, Effort, auth::Signer, http::Http};
use agent::{Model, ModelResponse, Sink, Thread};
use serde_json::Value;

/// Stateless Chat Completions adapter for hosted routers and local servers.
#[derive(Clone)]
pub struct ChatCompletionsModel {
    pub(crate) http: Http,
    pub(crate) parameters: crate::RequestParameters,
    pub(crate) model: String,
    pub(crate) effort: Option<Effort>,
    pub(crate) max_tokens: Option<u32>,
    /// Prompt capacity the caller declares for this target, in tokens.
    pub(crate) context_window: Option<usize>,
}

impl ChatCompletionsModel {
    /// # Errors
    /// Rejects invalid endpoints, empty model identifiers, and empty bearer tokens.
    pub fn new(
        endpoint: &str,
        model: impl Into<String>,
        bearer: impl Into<String>,
    ) -> Result<Self, ConfigError> {
        Self::build(endpoint, model.into(), Signer::Bearer(bearer.into()))
    }

    /// # Errors
    /// Rejects invalid endpoints and empty model identifiers.
    pub fn unauthenticated(endpoint: &str, model: impl Into<String>) -> Result<Self, ConfigError> {
        Self::build(endpoint, model.into(), Signer::Anonymous)
    }

    fn build(endpoint: &str, model: String, signer: Signer) -> Result<Self, ConfigError> {
        ConfigError::validate(&model, &signer)?;
        Ok(Self {
            http: Http::new(endpoint, signer)?,
            parameters: crate::RequestParameters::default(),
            model,
            effort: None,
            max_tokens: None,
            context_window: None,
        })
    }

    #[must_use]
    /// Declares this target's prompt capacity in tokens. Without it the runtime
    /// compacts on the configured budget alone and cannot tighten below it.
    pub const fn with_context_window(mut self, tokens: usize) -> Self {
        self.context_window = Some(tokens);
        self
    }

    #[must_use]
    /// Sets the reasoning effort the request asks for.
    pub const fn with_effort(mut self, effort: Effort) -> Self {
        self.effort = Some(effort);
        self
    }

    #[must_use]
    /// Caps each response's output tokens.
    pub const fn with_max_output_tokens(mut self, limit: u32) -> Self {
        self.max_tokens = Some(limit);
        self
    }
}

impl Model for ChatCompletionsModel {
    fn context_window(&self) -> Option<usize> {
        self.context_window
    }

    /// What the router publishes for this model, so a caller can size its
    /// budget before the first request.
    async fn discover_context_window(&self) -> Option<usize> {
        crate::model_catalog::discover(&self.http, &self.model).await
    }

    async fn generate(&self, thread: &Thread, tools: &[Value]) -> ModelResponse {
        crate::chat_call::generate(self, thread, tools)
            .await
            .unwrap_or_else(crate::failure::Failure::response)
    }

    async fn generate_stream<S: Sink + Send>(
        &self,
        thread: &Thread,
        tools: &[Value],
        sink: &mut S,
    ) -> Result<ModelResponse, S::Error> {
        crate::chat_stream::generate(self, thread, tools, sink).await
    }
}

impl std::fmt::Debug for ChatCompletionsModel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChatCompletionsModel")
            .field("endpoint", &self.http.redacted_endpoint().as_str())
            .field("model", &self.model)
            .field("credentials", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}
