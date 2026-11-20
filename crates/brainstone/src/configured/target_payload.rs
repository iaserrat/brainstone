use crate::configured::ConfigError;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;

pub(crate) fn headers(
    values: &HashMap<String, String>,
    name: &'static str,
) -> Result<HeaderMap, ConfigError> {
    let mut headers = HeaderMap::new();
    let Some(value) = values.get(name) else {
        return Ok(headers);
    };
    let fields: HashMap<String, String> =
        serde_json::from_str(value).map_err(|_| ConfigError::Invalid { name })?;
    for (key, value) in fields {
        let key = HeaderName::try_from(key).map_err(|_| ConfigError::Invalid { name })?;
        let mut value = HeaderValue::try_from(value).map_err(|_| ConfigError::Invalid { name })?;
        value.set_sensitive(true);
        headers.insert(key, value);
    }
    Ok(headers)
}

pub(crate) fn parameters(
    values: &HashMap<String, String>,
    name: &'static str,
) -> Result<openai::RequestParameters, ConfigError> {
    let Some(value) = values.get(name) else {
        return Ok(openai::RequestParameters::default());
    };
    let fields = serde_json::from_str(value).map_err(|_| ConfigError::Invalid { name })?;
    openai::RequestParameters::new(fields).map_err(|error| ConfigError::Detailed {
        name,
        message: error.to_string(),
    })
}
