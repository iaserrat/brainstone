use std::collections::HashMap;

use crate::ConfigError;
use crate::settings_parse::required;
use crate::settings_provider::{BEDROCK_APIS, ModelProvider};

/// The variables that configure one model target.
pub(crate) struct ProviderNames {
    pub(crate) provider: &'static str,
    pub(crate) api_key: &'static str,
    pub(crate) openai_key: &'static str,
    pub(crate) region: &'static str,
    /// AWS's standard service-specific endpoint override.
    pub(crate) endpoint: &'static str,
}

pub(crate) const PRIMARY: ProviderNames = ProviderNames {
    provider: "MODEL_PROVIDER",
    api_key: "ANTHROPIC_API_KEY",
    openai_key: "OPENAI_API_KEY",
    region: "AWS_REGION",
    endpoint: "AWS_ENDPOINT_URL_BEDROCK_RUNTIME",
};

pub(crate) fn model_provider(
    values: &HashMap<String, String>,
    names: &ProviderNames,
) -> Result<ModelProvider, ConfigError> {
    let api_key = |name| required(values, name);
    let api = match values.get(names.provider).map(String::as_str) {
        None | Some("anthropic") => {
            return Ok(ModelProvider::Anthropic {
                api_key: api_key(names.api_key)?,
            });
        }
        Some("openai") => {
            return Ok(ModelProvider::OpenAi {
                api_key: crate::target_validation::key(values, names)?,
            });
        }
        Some(value) => BEDROCK_APIS
            .iter()
            .find_map(|(name, api)| (*name == value).then_some(*api))
            .ok_or(ConfigError::Invalid {
                name: names.provider,
            })?,
    };
    Ok(ModelProvider::Bedrock {
        api,
        region: required(values, names.region)?,
        api_key: optional(values, names.api_key)?,
        endpoint_url: optional(values, names.endpoint)?,
    })
}

fn optional(
    values: &HashMap<String, String>,
    name: &'static str,
) -> Result<Option<String>, ConfigError> {
    values
        .contains_key(name)
        .then(|| required(values, name))
        .transpose()
}
