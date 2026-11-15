use std::collections::HashMap;

use crate::ConfigError;
use crate::settings_parse::flag;

/// The optional capability tools an operator switches on individually.
pub(crate) struct ToolToggles {
    pub(crate) silence: bool,
}

pub(crate) fn tool_toggles(values: &HashMap<String, String>) -> Result<ToolToggles, ConfigError> {
    Ok(ToolToggles {
        silence: flag(values, "SILENCE_TOOL")?,
    })
}
