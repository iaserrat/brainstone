use crate::auth::Signer;
use crate::config::ConfigError;
use crate::{Effort, http::Http};

/// An `OpenAI` Responses-compatible model endpoint.
#[derive(Clone)]
pub struct ResponsesModel {
    pub(crate) http: Http,
    pub(crate) parameters: crate::RequestParameters,
    pub(crate) model: String,
    pub(crate) effort: Option<Effort>,
    pub(crate) max_output_tokens: Option<u32>,
    pub(crate) reasoning_summary: bool,
}

impl ResponsesModel {
    /// Build an `OpenAI` model from `OPENAI_API_KEY` using the public endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::MissingEnvironment`] when the variable is
    /// absent or non-Unicode, and the same validation errors as [`Self::new`].
    pub fn openai(model: impl Into<String>) -> Result<Self, ConfigError> {
        let bearer = std::env::var("OPENAI_API_KEY")
            .map_err(|error| ConfigError::environment("OPENAI_API_KEY", &error))?;
        Self::new(crate::OPENAI_RESPONSES_URL, model, bearer)
    }

    /// Build a model against an exact `/responses` endpoint.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an invalid endpoint, empty model or token, or
    /// when the HTTP client cannot be constructed.
    pub fn new(
        endpoint: &str,
        model: impl Into<String>,
        bearer: impl Into<String>,
    ) -> Result<Self, ConfigError> {
        Self::build(endpoint, &model.into(), Signer::Bearer(bearer.into()))
    }

    pub(crate) fn build(endpoint: &str, model: &str, signer: Signer) -> Result<Self, ConfigError> {
        ConfigError::validate(model, &signer)?;
        Ok(Self {
            http: Http::new(endpoint, signer)?,
            parameters: crate::RequestParameters::default(),
            model: model.to_owned(),
            effort: None,
            max_output_tokens: None,
            reasoning_summary: false,
        })
    }

    /// Cap each response's output, reasoning included.
    #[must_use]
    pub const fn with_max_output_tokens(mut self, max_output_tokens: u32) -> Self {
        self.max_output_tokens = Some(max_output_tokens);
        self
    }

    #[must_use]
    /// Sets the reasoning effort the request asks for.
    pub const fn with_effort(mut self, effort: Effort) -> Self {
        self.effort = Some(effort);
        self
    }

    /// Ask for reasoning summaries whenever effort is set, and stream them as
    /// `ReasoningDelta`. `OpenAI` rejects this for unverified organizations.
    #[must_use]
    pub const fn with_reasoning_summary(mut self) -> Self {
        self.reasoning_summary = true;
        self
    }
}
