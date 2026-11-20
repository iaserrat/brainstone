use crate::configured::{
    ConfigError,
    settings_parse::required,
    target_names::Names,
    target_options::{Protocol, TargetOptions},
};
use reqwest::Url;
use std::collections::HashMap;

pub(crate) fn orphaned(
    values: &HashMap<String, String>,
    names: &Names,
) -> Result<Option<TargetOptions>, ConfigError> {
    if [
        names.protocol,
        names.target,
        names.auth,
        names.capabilities,
        names.headers,
        names.parameters,
        names.idle,
        names.reasoning_summary,
    ]
    .iter()
    .any(|name| values.contains_key(*name))
    {
        return Err(ConfigError::Missing {
            name: names.endpoint,
        });
    }
    Ok(None)
}

pub(crate) fn validate(values: &HashMap<String, String>, names: &Names) -> Result<(), ConfigError> {
    let url = Url::parse(&required(values, names.endpoint)?).map_err(|_| ConfigError::Invalid {
        name: names.endpoint,
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ConfigError::Invalid {
            name: names.endpoint,
        });
    }
    match values.get(names.auth).map(String::as_str) {
        None | Some("none" | "bearer") => Ok(()),
        _ => Err(ConfigError::Invalid { name: names.auth }),
    }
}

pub(crate) fn protocol(
    values: &HashMap<String, String>,
    names: &Names,
) -> Result<Protocol, ConfigError> {
    match values.get(names.protocol).map(String::as_str) {
        None | Some("responses") => Ok(Protocol::Responses),
        Some("chat-completions") => Ok(Protocol::ChatCompletions),
        _ => Err(ConfigError::Invalid {
            name: names.protocol,
        }),
    }
}

pub(crate) fn effort(
    values: &HashMap<String, String>,
    name: &'static str,
) -> Result<Option<openai::Effort>, ConfigError> {
    if !values.contains_key(name) {
        return Ok(None);
    }
    crate::configured::settings_parse::effort(values, name)
        .map(crate::configured::provider_effort::openai_effort)
        .map(Some)
}

pub(crate) fn output(
    values: &HashMap<String, String>,
    names: &Names,
) -> Result<Option<u32>, ConfigError> {
    let legacy = if names.output == "MODEL_MAX_OUTPUT_TOKENS" {
        "ANTHROPIC_MAX_TOKENS"
    } else {
        "FALLBACK_MAX_TOKENS"
    };
    [names.output, legacy]
        .into_iter()
        .find(|name| values.contains_key(*name))
        .map(|name| crate::configured::settings_parse::max_tokens(values, name))
        .transpose()
}
