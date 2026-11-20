use anthropic::Effort;
use std::collections::HashMap;

use crate::configured::ConfigError;

const DEFAULT_CONTEXT_WINDOW: usize = 200_000;

pub(crate) fn required(
    values: &HashMap<String, String>,
    name: &'static str,
) -> Result<String, ConfigError> {
    let value = values.get(name).ok_or(ConfigError::Missing { name })?;
    if value.trim().is_empty() {
        return Err(ConfigError::Invalid { name });
    }
    Ok(value.clone())
}

pub(crate) fn max_tokens(
    values: &HashMap<String, String>,
    name: &'static str,
) -> Result<u32, ConfigError> {
    required(values, name)?
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or(ConfigError::Invalid { name })
}

pub(crate) fn effort(
    values: &HashMap<String, String>,
    name: &'static str,
) -> Result<Effort, ConfigError> {
    let Some(value) = values.get(name) else {
        return Ok(Effort::High);
    };
    match value.as_str() {
        "low" => Ok(Effort::Low),
        "medium" => Ok(Effort::Medium),
        "high" => Ok(Effort::High),
        "xhigh" => Ok(Effort::XHigh),
        "max" => Ok(Effort::Max),
        _ => Err(ConfigError::Invalid { name }),
    }
}

pub(crate) fn compaction_limit(values: &HashMap<String, String>) -> Result<usize, ConfigError> {
    let context = positive(values, "MODEL_CONTEXT_WINDOW", DEFAULT_CONTEXT_WINDOW)?;
    let safe = context
        .checked_mul(9)
        .map(|value| value / 10)
        .filter(|value| *value > 0)
        .ok_or(ConfigError::Invalid {
            name: "MODEL_CONTEXT_WINDOW",
        })?;
    let limit = positive(values, "AUTO_COMPACT_TOKEN_LIMIT", safe)?;
    Ok(limit.min(safe))
}

pub(crate) fn positive(
    values: &HashMap<String, String>,
    name: &'static str,
    default: usize,
) -> Result<usize, ConfigError> {
    let Some(value) = values.get(name) else {
        return Ok(default);
    };
    value
        .parse()
        .ok()
        .filter(|value| *value > 0)
        .ok_or(ConfigError::Invalid { name })
}

/// A positive minute count that defaults to the schedule interval floor.
pub(crate) fn minutes(
    values: &HashMap<String, String>,
    name: &'static str,
) -> Result<u32, ConfigError> {
    let default = policy::DEFAULT_MIN_INTERVAL_MINUTES as usize;
    u32::try_from(positive(values, name, default)?).map_err(|_| ConfigError::Invalid { name })
}

pub(crate) fn flag(
    values: &HashMap<String, String>,
    name: &'static str,
) -> Result<bool, ConfigError> {
    match values.get(name).map(String::as_str) {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        Some(_) => Err(ConfigError::Invalid { name }),
    }
}
