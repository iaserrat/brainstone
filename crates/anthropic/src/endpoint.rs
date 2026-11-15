//! Where Messages requests go: an SSE Messages URL or the Bedrock runtime.

use std::fmt;

use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::config::{Region, Token};
use aws_smithy_runtime_api::client::auth::http::HTTP_BEARER_AUTH_SCHEME_ID;
use reqwest::Url;

use crate::{Auth, ConfigError};

#[derive(Clone)]
/// Where Messages requests go.
pub enum Endpoint {
    /// An SSE Messages endpoint (Anthropic API or Claude in Amazon Bedrock),
    /// authenticated per request by `Auth`.
    Messages(Url),
    /// Amazon Bedrock runtime (`InvokeModelWithResponseStream`); the client
    /// already carries its credentials, so the model's `Auth` is unused.
    BedrockRuntime(Client),
}

impl Endpoint {
    /// # Errors
    ///
    /// Returns `invalid_endpoint` unless `url` is an HTTP or HTTPS URL.
    pub fn messages(url: &str) -> Result<Self, ConfigError> {
        parse(url).map(Self::Messages)
    }

    /// Claude in Amazon Bedrock (Messages API) for `region`.
    ///
    /// # Errors
    ///
    /// Returns `empty_region` for a blank region.
    pub fn bedrock_mantle(region: &str) -> Result<Self, ConfigError> {
        let region = non_empty(region)?;
        Self::messages(&format!(
            "https://bedrock-mantle.{region}.api.aws/anthropic/v1/messages"
        ))
    }

    /// Amazon Bedrock runtime for `region`, optionally at `url` (a VPC or
    /// partition endpoint). `ApiKey` is a bearer token; `SigV4` signs.
    ///
    /// # Errors
    ///
    /// Returns `empty_region`, `empty_api_key`, or `invalid_endpoint`.
    pub fn bedrock_runtime(
        region: &str,
        url: Option<&str>,
        auth: &Auth,
    ) -> Result<Self, ConfigError> {
        let region = non_empty(region)?;
        let mut config = aws_sdk_bedrockruntime::Config::builder()
            .behavior_version_latest()
            .region(Region::new(region.to_owned()));
        config = match auth {
            Auth::ApiKey(key) if key.is_empty() => return Err(ConfigError::EmptyApiKey),
            Auth::ApiKey(key) => config
                .token_provider(Token::new(key.clone(), None))
                .auth_scheme_preference([HTTP_BEARER_AUTH_SCHEME_ID]),
            Auth::SigV4(sigv4) => config.credentials_provider(sigv4.provider().clone()),
        };
        if let Some(url) = url {
            config = config.endpoint_url(parse(url)?.as_str());
        }
        Ok(Self::BedrockRuntime(Client::from_conf(config.build())))
    }
}

impl fmt::Debug for Endpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Messages(url) => write!(formatter, "Messages({url})"),
            Self::BedrockRuntime(client) => {
                write!(formatter, "BedrockRuntime({:?})", client.config().region())
            }
        }
    }
}

fn parse(url: &str) -> Result<Url, ConfigError> {
    let url = Url::parse(url).map_err(|error| ConfigError::InvalidEndpoint {
        message: error.to_string(),
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ConfigError::InvalidEndpoint {
            message: "endpoint must use HTTP or HTTPS".to_owned(),
        });
    }
    Ok(url)
}

fn non_empty(region: &str) -> Result<&str, ConfigError> {
    let region = region.trim();
    if region.is_empty() {
        return Err(ConfigError::EmptyRegion);
    }
    Ok(region)
}
