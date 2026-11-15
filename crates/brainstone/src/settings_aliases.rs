use crate::ConfigError;
use std::{borrow::Cow, collections::HashMap};

/// Preserve legacy names and error messages; neutral names are additive aliases.
pub(crate) fn normalize(
    values: &HashMap<String, String>,
) -> Result<Cow<'_, HashMap<String, String>>, ConfigError> {
    let mut normalized = Cow::Borrowed(values);
    for (name, legacy) in [
        ("MODEL_NAME", "ANTHROPIC_MODEL"),
        ("MODEL_MAX_OUTPUT_TOKENS", "ANTHROPIC_MAX_TOKENS"),
        ("FALLBACK_MAX_OUTPUT_TOKENS", "FALLBACK_MAX_TOKENS"),
    ] {
        if let Some(value) = values.get(name) {
            if values.get(legacy).is_some_and(|old| old != value) {
                return Err(ConfigError::Invalid { name });
            }
            normalized.to_mut().insert(legacy.to_owned(), value.clone());
        }
    }
    if values.contains_key("MODEL_ENDPOINT") && !normalized.contains_key("ANTHROPIC_MAX_TOKENS") {
        normalized
            .to_mut()
            .insert("ANTHROPIC_MAX_TOKENS".to_owned(), "8192".to_owned());
    }
    Ok(normalized)
}
