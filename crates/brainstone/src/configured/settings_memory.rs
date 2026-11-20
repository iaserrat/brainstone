use std::collections::HashMap;

use crate::configured::ConfigError;
use crate::configured::settings_parse::{flag, positive};

pub(crate) fn memory(values: &HashMap<String, String>) -> Result<memory::Config, ConfigError> {
    let defaults = memory::Config::default();
    let inject = positive(values, "MEMORY_INJECT", 0)?;
    Ok(memory::Config {
        scope: memory_scope(values)?,
        save: flag(values, "MEMORY_SAVE")?,
        recall_tool: flag(values, "MEMORY_RECALL_TOOL")?,
        recall_limit: positive(values, "MEMORY_RECALL_LIMIT", defaults.recall_limit)?,
        inject: (inject > 0).then_some(inject),
    })
}

fn memory_scope(values: &HashMap<String, String>) -> Result<memory::Scope, ConfigError> {
    match values.get("MEMORY_SCOPE").map(String::as_str) {
        None | Some("conversation") => Ok(memory::Scope::Conversation),
        Some("surface") => Ok(memory::Scope::Surface),
        Some("user") => Ok(memory::Scope::User),
        Some(_) => Err(ConfigError::Invalid {
            name: "MEMORY_SCOPE",
        }),
    }
}
