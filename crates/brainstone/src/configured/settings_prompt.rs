use std::io::ErrorKind;
use std::path::Path;

use crate::configured::ConfigError;

const SYSTEM_PROMPT_PATH: &str = "prompts/system_prompt.md";

pub(crate) fn system_prompt(root: &Path) -> Result<String, ConfigError> {
    let path = root.join(SYSTEM_PROMPT_PATH);
    let prompt = std::fs::read_to_string(path).map_err(|error| match error.kind() {
        ErrorKind::NotFound => ConfigError::Missing {
            name: SYSTEM_PROMPT_PATH,
        },
        _ => ConfigError::Unreadable {
            name: SYSTEM_PROMPT_PATH,
        },
    })?;
    if prompt.trim().is_empty() {
        return Err(ConfigError::Invalid {
            name: SYSTEM_PROMPT_PATH,
        });
    }
    Ok(prompt)
}
