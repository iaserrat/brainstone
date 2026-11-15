use anthropic::Effort;

use crate::{CoreSettings, FallbackModel, ModelProvider};

impl CoreSettings {
    /// Where model requests go and how they authenticate.
    #[must_use]
    pub const fn model_provider(&self) -> &ModelProvider {
        &self.model_provider
    }

    /// The primary model id.
    #[must_use]
    pub fn anthropic_model(&self) -> &str {
        &self.anthropic_model
    }

    /// The secondary model, when failover is configured.
    #[must_use]
    pub const fn fallback(&self) -> Option<&FallbackModel> {
        self.fallback.as_ref()
    }

    /// The primary model's output cap.
    #[must_use]
    pub const fn anthropic_max_tokens(&self) -> u32 {
        self.anthropic_max_tokens
    }

    /// The primary model's reasoning effort.
    #[must_use]
    pub const fn model_effort(&self) -> Effort {
        self.model_effort
    }

    /// `PROMPT_CACHE`: automatic prompt caching on Messages requests.
    #[must_use]
    pub const fn prompt_cache(&self) -> bool {
        self.prompt_cache
    }

    /// The configured response limit; `None` means unlimited.
    #[must_use]
    pub const fn max_turns(&self) -> Option<usize> {
        self.max_turns
    }

    /// The shortest fixed interval a standing schedule may use.
    #[must_use]
    pub const fn schedule_min_interval_minutes(&self) -> u32 {
        self.schedule_min_interval_minutes
    }

    /// The token count that triggers automatic compaction.
    #[must_use]
    pub const fn auto_compact_token_limit(&self) -> usize {
        self.auto_compact_token_limit
    }

    /// The system prompt loaded at startup.
    #[must_use]
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// The cross-session memory configuration.
    #[must_use]
    pub const fn memory(&self) -> &memory::Config {
        &self.memory
    }

    /// The authenticated actors admission permits, or `None` for all.
    #[must_use]
    pub fn allowed_actors(&self) -> Option<&[String]> {
        self.admission.allowed_actors.as_deref()
    }

    /// The daily operation cap per actor, or `None` for no cap.
    #[must_use]
    pub const fn actor_daily_limit(&self) -> Option<usize> {
        self.admission.actor_daily_limit
    }

    /// The executable that chooses each prompt's model and effort, if configured.
    #[must_use]
    pub const fn model_selector(&self) -> Option<&host::ModelSelector> {
        self.admission.model_selector.as_ref()
    }

    /// The identity this process uses as a session's single writer.
    #[must_use]
    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }
}
