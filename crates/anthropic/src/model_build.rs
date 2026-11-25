use std::time::Duration;

use agent::Egress;
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
    Ok(AnthropicModel {
        client: client(&Egress::Direct)?,
        endpoint,
        model: model.to_owned(),
        auth,
        max_tokens,
        effort: None,
        prompt_cache: false,
    })
}

/// The client the adapter sends with: one route, one timeout, no redirects.
pub(crate) fn client(egress: &Egress) -> Result<Client, ConfigError> {
    let builder = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(IDLE_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none());
    let builder = match egress.proxy_url() {
        Some(url) => builder.proxy(proxy(url)?),
        None => builder,
    };
    builder.build().map_err(|error| ConfigError::ClientBuild {
        message: error.to_string(),
    })
}

/// Only an HTTP proxy is usable: the client is built without SOCKS support, and
/// reqwest refuses an unknown scheme on the first request instead of here.
fn proxy(url: &str) -> Result<reqwest::Proxy, ConfigError> {
    let invalid = |message: String| ConfigError::InvalidProxy { message };
    let parsed = reqwest::Url::parse(url).map_err(|error| invalid(error.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(invalid(format!("{url} is not an http or https proxy URL")));
    }
    reqwest::Proxy::all(parsed).map_err(|error| invalid(error.to_string()))
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
