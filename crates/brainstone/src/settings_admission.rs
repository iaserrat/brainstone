use std::collections::HashMap;

use crate::ConfigError;
use crate::settings_model_selector::model_selector;
use crate::settings_parse::positive;

/// Settings that only the admission chain reads.
pub(crate) struct Admission {
    pub(crate) allowed_actors: Option<Vec<String>>,
    pub(crate) actor_daily_limit: Option<usize>,
    pub(crate) model_selector: Option<host::ModelSelector>,
}

pub(crate) fn admission(values: &HashMap<String, String>) -> Result<Admission, ConfigError> {
    Ok(Admission {
        allowed_actors: allowed_actors(values)?,
        actor_daily_limit: actor_daily_limit(values)?,
        model_selector: model_selector(values)?,
    })
}

/// Comma-separated surface actor ids admitted to prompt the agent; unset
/// leaves admission open. Set but empty is a misconfiguration, not lockdown.
fn allowed_actors(values: &HashMap<String, String>) -> Result<Option<Vec<String>>, ConfigError> {
    ids(values, "ADMISSION_ALLOWED_ACTORS")
}

/// A comma-separated id allowlist; unset allows everything, set but empty is
/// a misconfiguration.
pub(crate) fn ids(
    values: &HashMap<String, String>,
    name: &'static str,
) -> Result<Option<Vec<String>>, ConfigError> {
    let Some(value) = values.get(name) else {
        return Ok(None);
    };
    let ids: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    if ids.is_empty() {
        return Err(ConfigError::Invalid { name });
    }
    Ok(Some(ids))
}

/// Most operations one actor may have accepted per trailing day; unset
/// leaves admission unmetered.
fn actor_daily_limit(values: &HashMap<String, String>) -> Result<Option<usize>, ConfigError> {
    let limit = positive(values, "ADMISSION_ACTOR_DAILY_LIMIT", 0)?;
    Ok((limit > 0).then_some(limit))
}
