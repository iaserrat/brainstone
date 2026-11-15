use crate::CoreSettings;

impl CoreSettings {
    /// Provider-neutral name. `anthropic_model()` remains a compatible alias.
    #[must_use]
    pub fn model_name(&self) -> &str {
        &self.anthropic_model
    }

    /// The stable target identity used to scope provider continuation state.
    #[must_use]
    pub fn model_target(&self) -> String {
        self.targets.primary.as_ref().map_or_else(
            || self.model_provider.target(),
            |target| target.target.clone(),
        )
    }

    /// Custom targets omit the output limit unless the operator supplied one.
    #[must_use]
    pub fn max_output_tokens(&self) -> Option<u32> {
        self.targets
            .primary
            .as_ref()
            .map_or(Some(self.anthropic_max_tokens), |target| target.output)
    }

    /// Legacy targets retain their existing default effort.
    #[must_use]
    pub fn reasoning_effort(&self) -> Option<openai::Effort> {
        self.targets.primary.as_ref().map_or(
            Some(crate::provider_effort::openai_effort(self.model_effort)),
            |target| target.effort,
        )
    }

    /// The primary target's declared capabilities, or the defaults.
    #[must_use]
    pub fn model_capabilities(&self) -> agent::ModelCapabilities {
        self.targets
            .primary
            .as_ref()
            .map_or_else(agent::ModelCapabilities::default, |target| {
                target.capabilities
            })
    }
}
