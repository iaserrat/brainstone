use std::fmt;

use crate::configured::{CoreSettings, FallbackModel, Settings};

impl fmt::Debug for CoreSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CoreSettings")
            .field("model_provider", &self.model_provider.redacted())
            .field("anthropic_model", &self.anthropic_model)
            .field("fallback", &self.fallback.as_ref().map(FallbackModel::name))
            .field("anthropic_max_tokens", &self.anthropic_max_tokens)
            .field("model_effort", &self.model_effort)
            .field("model_selector", &self.admission.model_selector)
            .field("prompt_cache", &self.prompt_cache)
            .field("max_turns", &self.max_turns)
            .field("worker_id", &self.worker_id)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for Settings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Settings")
            .field("core", &self.core)
            .finish_non_exhaustive()
    }
}
