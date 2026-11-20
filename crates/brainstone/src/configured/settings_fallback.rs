use std::collections::HashMap;

use crate::configured::ConfigError;
use crate::configured::settings_fallback_model::FallbackModel;
use crate::configured::settings_parse::{effort, max_tokens, required};
use crate::configured::settings_provider_parse::{PRIMARY, ProviderNames, model_provider};

const MODEL: &str = "FALLBACK_MODEL";
const MAX_TOKENS: &str = "FALLBACK_MAX_TOKENS";
const EFFORT: &str = "FALLBACK_MODEL_EFFORT";
const NAMES: ProviderNames = ProviderNames {
    provider: "FALLBACK_MODEL_PROVIDER",
    api_key: "FALLBACK_ANTHROPIC_API_KEY",
    openai_key: "FALLBACK_OPENAI_API_KEY",
    region: "FALLBACK_AWS_REGION",
    endpoint: "FALLBACK_AWS_ENDPOINT_URL_BEDROCK_RUNTIME",
};

/// `None` unless `FALLBACK_MODEL` is set. Any other `FALLBACK_*` variable
/// without it is a mistake that would silently disable failover.
pub(crate) fn fallback(
    values: &HashMap<String, String>,
) -> Result<Option<FallbackModel>, ConfigError> {
    if !values.contains_key(MODEL) {
        return orphaned(values);
    }
    let fallback = FallbackModel {
        provider: model_provider(values, &NAMES)?,
        model: required(values, MODEL)?,
        max_tokens: max_tokens(values, own(values, MAX_TOKENS, "ANTHROPIC_MAX_TOKENS"))?,
        effort: effort(values, own(values, EFFORT, "MODEL_EFFORT"))?,
    };
    let primary = model_provider(values, &PRIMARY)?;
    if crate::configured::target_validation::name(
        values,
        &crate::configured::target_names::FALLBACK,
        &fallback.name(),
    )? == crate::configured::target_validation::name(
        values,
        &crate::configured::target_names::PRIMARY,
        &primary.named(&required(values, "ANTHROPIC_MODEL")?),
    )? {
        return Err(ConfigError::Detailed {
            name: MODEL,
            message: "must differ from the primary in provider, region, or model".to_owned(),
        });
    }
    Ok(Some(fallback))
}

fn orphaned(values: &HashMap<String, String>) -> Result<Option<FallbackModel>, ConfigError> {
    let names = [
        NAMES.provider,
        NAMES.api_key,
        NAMES.openai_key,
        NAMES.region,
        NAMES.endpoint,
        MAX_TOKENS,
        EFFORT,
        "FALLBACK_MODEL_ENDPOINT",
        "FALLBACK_MAX_OUTPUT_TOKENS",
    ];
    if names.iter().any(|name| values.contains_key(*name)) {
        return Err(ConfigError::Missing { name: MODEL });
    }
    Ok(None)
}

/// The fallback's own variable when set, else the primary's.
fn own(
    values: &HashMap<String, String>,
    name: &'static str,
    primary: &'static str,
) -> &'static str {
    if values.contains_key(name) {
        name
    } else {
        primary
    }
}
