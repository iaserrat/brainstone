use std::collections::HashMap;

use crate::ConfigError;

/// Omission is unlimited; a supplied value must be positive or `unlimited`.
pub(crate) fn parse(values: &HashMap<String, String>) -> Result<Option<usize>, ConfigError> {
    values.get("MAX_TURNS").map_or(Ok(None), |value| {
        value
            .parse::<agent::TurnLimit>()
            .map(agent::TurnLimit::get)
            .map_err(|_| ConfigError::Invalid { name: "MAX_TURNS" })
    })
}
