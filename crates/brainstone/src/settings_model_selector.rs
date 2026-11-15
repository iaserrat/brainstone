use std::collections::HashMap;
use std::path::Path;

use crate::ConfigError;

const COMMAND: &str = "DYNAMIC_MODEL_AND_EFFORT_COMMAND";
const ARGS: &str = "DYNAMIC_MODEL_AND_EFFORT_ARGS";

/// The executable that picks each prompt's model and effort. Unset keeps the
/// configured model and effort for every prompt; set both or neither.
pub(crate) fn model_selector(
    values: &HashMap<String, String>,
) -> Result<Option<host::ModelSelector>, ConfigError> {
    match (values.get(COMMAND), values.get(ARGS)) {
        (None, None) => Ok(None),
        (Some(command), Some(args)) => selector(command, args).map(Some),
        (None, Some(_)) => Err(ConfigError::Missing { name: COMMAND }),
        (Some(_), None) => Err(ConfigError::Missing { name: ARGS }),
    }
}

fn selector(command: &str, args: &str) -> Result<host::ModelSelector, ConfigError> {
    if !Path::new(command).is_absolute() {
        return Err(ConfigError::Invalid { name: COMMAND });
    }
    let args = serde_json::from_str(args).map_err(|_| ConfigError::Invalid { name: ARGS })?;
    Ok(host::ModelSelector::new(command.to_owned(), args))
}
