use anthropic::Effort;

use crate::configured::settings_provider::ModelProvider;

/// The secondary model a turn fails over to when the primary is unavailable.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FallbackModel {
    pub(crate) provider: ModelProvider,
    pub(crate) model: String,
    pub(crate) max_tokens: u32,
    pub(crate) effort: Effort,
}

impl FallbackModel {
    /// The fallback's provider and credentials.
    #[must_use]
    pub const fn provider(&self) -> &ModelProvider {
        &self.provider
    }

    /// The fallback's model id.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// `FALLBACK_MAX_TOKENS`, else the primary's `ANTHROPIC_MAX_TOKENS`.
    #[must_use]
    pub const fn max_tokens(&self) -> u32 {
        self.max_tokens
    }

    /// `FALLBACK_MODEL_EFFORT`, else the primary's `MODEL_EFFORT`.
    #[must_use]
    pub const fn effort(&self) -> Effort {
        self.effort
    }

    pub(crate) fn name(&self) -> String {
        self.provider.named(&self.model)
    }
}
