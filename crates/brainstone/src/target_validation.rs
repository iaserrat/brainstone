use crate::{
    ConfigError,
    settings_parse::required,
    settings_provider_parse::ProviderNames,
    target_names::{FALLBACK, Names, PRIMARY},
    target_options::TargetOptions,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt::Write as _;

pub(crate) fn key(
    values: &HashMap<String, String>,
    provider: &ProviderNames,
) -> Result<String, ConfigError> {
    let names = if provider.provider == "MODEL_PROVIDER" {
        &PRIMARY
    } else {
        &FALLBACK
    };
    if values.get(names.auth).is_some_and(|value| value == "none") {
        required(values, names.endpoint)?;
        if values
            .get(provider.openai_key)
            .is_some_and(|key| !key.trim().is_empty())
        {
            return Err(ConfigError::Invalid { name: names.auth });
        }
        return Ok(String::new());
    }
    required(values, provider.openai_key)
}

pub(crate) fn providers(
    values: &HashMap<String, String>,
    primary: Option<&TargetOptions>,
    fallback: Option<&TargetOptions>,
) -> Result<(), ConfigError> {
    for (options, provider, names) in [
        (primary, "MODEL_PROVIDER", &PRIMARY),
        (fallback, "FALLBACK_MODEL_PROVIDER", &FALLBACK),
    ] {
        if options.is_some() && values.get(provider).map(String::as_str) != Some("openai") {
            return Err(ConfigError::Invalid { name: provider });
        }
        if let Some(options) = options
            && options.effort.is_some()
            && !options.capabilities.reasoning_effort
        {
            return Err(ConfigError::Invalid { name: names.effort });
        }
    }
    Ok(())
}

/// Endpoint and protocol are part of replay identity; secrets stay out of records.
pub(crate) fn identity(
    values: &HashMap<String, String>,
    names: &Names,
) -> Result<String, ConfigError> {
    let endpoint = required(values, names.endpoint)?;
    let protocol = values
        .get(names.protocol)
        .map_or("responses", String::as_str);
    let digest = Sha256::digest(format!("{protocol}\n{endpoint}"));
    let digest = digest.iter().fold(String::new(), |mut hex, byte| {
        write!(&mut hex, "{byte:02x}").expect("writing to String");
        hex
    });
    let target = required(values, names.target)?;
    Ok(format!("compatible/{target}/{digest}"))
}

pub(crate) fn name(
    values: &HashMap<String, String>,
    names: &Names,
    legacy: &str,
) -> Result<String, ConfigError> {
    if !values.contains_key(names.endpoint) {
        return Ok(legacy.to_owned());
    }
    let model = legacy.split_once('/').expect("named provider and model").1;
    Ok(format!("{}/{model}", identity(values, names)?))
}
