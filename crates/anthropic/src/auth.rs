//! Request credentials: an API key header or AWS `SigV4` signing.

use std::fmt;

use reqwest::Url;

use sigv4::SigV4;

use crate::failure::{AUTHENTICATION_FAILED, Failure};

const API_KEY_HEADER: &str = "x-api-key";
/// AWS service name of Claude in Amazon Bedrock (Messages API).
const BEDROCK_SERVICE: &str = "bedrock-mantle";

#[derive(Clone)]
/// How one Messages request authenticates.
pub enum Auth {
    /// An Anthropic API key, or a Bedrock API key (bearer token).
    ApiKey(String),
    /// AWS IAM identity from the standard credential chain.
    SigV4(SigV4),
}

impl Auth {
    /// Headers that authenticate one Messages request. `SigV4` signs `base`
    /// headers and `body`, so callers must send exactly those alongside.
    pub(crate) async fn headers(
        &self,
        url: &Url,
        base: &[(&str, &str)],
        body: &[u8],
    ) -> Result<Vec<(String, String)>, Failure> {
        match self {
            Self::ApiKey(key) => Ok(vec![(API_KEY_HEADER.to_owned(), key.clone())]),
            Self::SigV4(sigv4) => sigv4
                .headers(BEDROCK_SERVICE, url.as_str(), base, body)
                .await
                .map_err(|error| Failure::new(AUTHENTICATION_FAILED, error.to_string())),
        }
    }
}

impl fmt::Debug for Auth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey(_) => formatter.write_str("ApiKey([REDACTED])"),
            Self::SigV4(sigv4) => write!(formatter, "SigV4({sigv4:?})"),
        }
    }
}
