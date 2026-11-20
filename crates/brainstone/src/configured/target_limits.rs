use crate::configured::{ConfigError, settings_parse::positive};
use std::{collections::HashMap, num::NonZeroUsize, time::Duration};

const DEFAULT_IDLE_SECONDS: usize = 120;

pub(crate) fn idle(
    values: &HashMap<String, String>,
    name: &'static str,
) -> Result<Duration, ConfigError> {
    let seconds = u64::try_from(positive(values, name, DEFAULT_IDLE_SECONDS)?)
        .map_err(|_| ConfigError::Invalid { name })?;
    Ok(Duration::from_secs(seconds))
}

pub(crate) fn context(
    values: &HashMap<String, String>,
    name: &'static str,
) -> Result<Option<NonZeroUsize>, ConfigError> {
    if !values.contains_key(name) {
        return Ok(None);
    }
    positive(values, name, 1).map(NonZeroUsize::new)
}
