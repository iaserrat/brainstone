use std::collections::HashMap;
#[path = "core_settings_codec.rs"]
mod codec;
use codec::{detailed, ordered};
use std::path::Path;

use anthropic::Effort;

use crate::ConfigError;
use crate::settings_admission::{Admission, admission};
use crate::settings_fallback::fallback;
use crate::settings_fallback_model::FallbackModel;
use crate::settings_memory::memory;
use crate::settings_parse::{compaction_limit, effort, flag, max_tokens, minutes, required};
use crate::settings_prompt::system_prompt;
use crate::settings_provider::ModelProvider;
use crate::settings_provider_parse::{PRIMARY, model_provider};
use crate::settings_tools::{ToolToggles, tool_toggles};

/// The settings every component shares, parsed once at startup.
pub struct CoreSettings {
    pub(crate) tool_policy: Option<Box<runtime::ToolPolicy>>,
    pub(crate) database_url: String,
    pub(crate) targets: Box<crate::settings_targets::Targets>,
    pub(crate) model_provider: ModelProvider,
    pub(crate) anthropic_model: String,
    pub(crate) fallback: Option<FallbackModel>,
    pub(crate) anthropic_max_tokens: u32,
    pub(crate) model_effort: Effort,
    pub(crate) prompt_cache: bool,
    pub(crate) max_turns: Option<usize>,
    pub(crate) schedule_min_interval_minutes: u32,
    pub(crate) auto_compact_token_limit: usize,
    pub(crate) mcp: mcp::Config,
    pub(crate) memory: memory::Config,
    pub(crate) skills: skills::Catalog,
    pub(crate) subagents: subagents::Catalog,
    pub(crate) system_prompt: String,
    pub(crate) admission: Admission,
    pub(crate) tools: ToolToggles,
    pub(crate) worker_id: String,
}

impl CoreSettings {
    /// Load surface-independent configuration below `root`.
    ///
    /// # Errors
    /// Returns `invalid_configuration` for missing or malformed input.
    pub fn from_map(values: &HashMap<String, String>, root: &Path) -> Result<Self, ConfigError> {
        let parsed = crate::settings_targets::parse(values)?;
        Self::load(&parsed.values, root, parsed.targets)
    }

    fn load(
        values: &HashMap<String, String>,
        root: &Path,
        targets: Box<crate::settings_targets::Targets>,
    ) -> Result<Self, ConfigError> {
        let mcp = mcp::Config::load(root, &ordered(values)).map_err(detailed("mcp.toml"))?;
        Ok(Self {
            tool_policy: crate::settings_tool_policy::load(root, &mcp)?,
            targets,
            database_url: required(values, "DATABASE_URL")?,
            model_provider: model_provider(values, &PRIMARY)?,
            anthropic_model: required(values, "ANTHROPIC_MODEL")?,
            fallback: fallback(values)?,
            anthropic_max_tokens: max_tokens(values, "ANTHROPIC_MAX_TOKENS")?,
            model_effort: effort(values, "MODEL_EFFORT")?,
            prompt_cache: flag(values, "PROMPT_CACHE")?,
            max_turns: crate::settings_turn_limit::parse(values)?,
            schedule_min_interval_minutes: minutes(values, "SCHEDULE_MIN_INTERVAL_MINUTES")?,
            auto_compact_token_limit: compaction_limit(values)?,
            mcp,
            memory: memory(values)?,
            skills: skills::Catalog::load(&root.join("skills")),
            subagents: subagents::Catalog::load(&root.join("agents"))
                .map_err(detailed("agents"))?,
            system_prompt: system_prompt(root)?,
            admission: admission(values)?,
            tools: tool_toggles(values)?,
            worker_id: required(values, "WORKER_ID")?,
        })
    }

    /// # Errors
    /// Returns `invalid_configuration` for invalid environment or files.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_map(&std::env::vars().collect(), Path::new("."))
    }
}
