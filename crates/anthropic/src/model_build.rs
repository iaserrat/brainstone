use std::time::Duration;

use reqwest::Client;

use crate::{AnthropicModel, Auth, ConfigError, Endpoint};

/// Connection setup is bounded, but the streamed body is bounded only by
/// silence between bytes, so a long turn never times out while events flow.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(not(test))]
pub(crate) const IDLE_TIMEOUT: Duration = Duration::from_mins(2);
#[cfg(test)]
pub(crate) const IDLE_TIMEOUT: Duration = Duration::from_millis(500);

pub(crate) fn build(
    endpoint: Endpoint,
    model: &str,
    auth: Auth,
    max_tokens: u32,
) -> Result<AnthropicModel, ConfigError> {
    validate(model, &auth, max_tokens)?;
    let client = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(IDLE_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| ConfigError::ClientBuild {
            message: error.to_string(),
        })?;
    Ok(AnthropicModel {
        client,
        endpoint,
        model: model.to_owned(),
        auth,
        max_tokens,
        effort: None,
        prompt_cache: false,
    })
}

fn validate(model: &str, auth: &Auth, max_tokens: u32) -> Result<(), ConfigError> {
    if model.is_empty() {
        return Err(ConfigError::EmptyModel);
    }
    if matches!(auth, Auth::ApiKey(key) if key.is_empty()) {
        return Err(ConfigError::EmptyApiKey);
    }
    if max_tokens == 0 {
        return Err(ConfigError::InvalidMaxTokens);
    }
    Ok(())
}
