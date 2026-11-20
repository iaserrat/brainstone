use anthropic::Effort;

use crate::configured::{ModelProvider, Settings};

impl Settings {
    /// The settings every component shares.
    #[must_use]
    pub const fn core(&self) -> &crate::configured::CoreSettings {
        &self.core
    }

    /// Where model requests go and how they authenticate.
    // software-design-allow: pass-through-method preserves the existing public Settings API.
    #[must_use]
    pub const fn model_provider(&self) -> &ModelProvider {
        self.core.model_provider()
    }

    /// The primary model id.
    // software-design-allow: pass-through-method preserves the existing public Settings API.
    #[must_use]
    pub fn anthropic_model(&self) -> &str {
        self.core.anthropic_model()
    }

    /// The primary model's output cap.
    // software-design-allow: pass-through-method preserves the existing public Settings API.
    #[must_use]
    pub const fn anthropic_max_tokens(&self) -> u32 {
        self.core.anthropic_max_tokens()
    }

    /// The primary model's reasoning effort.
    // software-design-allow: pass-through-method preserves the existing public Settings API.
    #[must_use]
    pub const fn model_effort(&self) -> Effort {
        self.core.model_effort()
    }

    /// Whether Messages requests use automatic prompt caching.
    // software-design-allow: pass-through-method preserves the existing public Settings API.
    #[must_use]
    pub const fn prompt_cache(&self) -> bool {
        self.core.prompt_cache()
    }

    /// The configured response limit; `None` means unlimited.
    // software-design-allow: pass-through-method Retain the existing public Settings facade.
    #[must_use]
    pub const fn max_turns(&self) -> Option<usize> {
        self.core.max_turns()
    }

    /// The token count that triggers automatic compaction.
    // software-design-allow: pass-through-method preserves the existing public Settings API.
    #[must_use]
    pub const fn auto_compact_token_limit(&self) -> usize {
        self.core.auto_compact_token_limit()
    }

    /// The system prompt loaded at startup.
    // software-design-allow: pass-through-method preserves the existing public Settings API.
    #[must_use]
    pub fn system_prompt(&self) -> &str {
        self.core.system_prompt()
    }

    /// The cross-session memory configuration.
    // software-design-allow: pass-through-method preserves the existing public Settings API.
    #[must_use]
    pub const fn memory(&self) -> &memory::Config {
        self.core.memory()
    }

    /// The authenticated actors admission permits, or `None` for all.
    // software-design-allow: pass-through-method preserves the existing public Settings API.
    #[must_use]
    pub fn allowed_actors(&self) -> Option<&[String]> {
        self.core.allowed_actors()
    }

    /// The daily operation cap per actor, or `None` for no cap.
    // software-design-allow: pass-through-method preserves the existing public Settings API.
    #[must_use]
    pub const fn actor_daily_limit(&self) -> Option<usize> {
        self.core.actor_daily_limit()
    }

    /// Whether the `Silence` tool is offered.
    #[must_use]
    pub const fn silence_tool(&self) -> bool {
        self.core.tools.silence
    }

    /// Whether `CreateFile` is offered.
    #[must_use]
    pub const fn file_tool(&self) -> bool {
        self.file_tool
    }
}
