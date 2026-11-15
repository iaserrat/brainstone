//! Request credentials: a bearer token or AWS `SigV4` signing.

use reqwest::RequestBuilder;
use sigv4::SigV4;

use crate::failure::{CODE_AUTHENTICATION, Failure};

/// How a model authenticates its requests.
#[derive(Clone)]
pub enum Auth {
    /// An `OpenAI` API key, or an Amazon Bedrock API key.
    Bearer(String),
    /// AWS IAM identity from the standard credential chain.
    SigV4(SigV4),
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bearer(_) => formatter.write_str("Bearer(<redacted>)"),
            Self::SigV4(_) => formatter.write_str("SigV4(..)"),
        }
    }
}

/// [`Auth`] bound to the AWS service name it signs for.
#[derive(Clone)]
pub(crate) enum Signer {
    Anonymous,
    Bearer(String),
    SigV4(SigV4, &'static str),
}

impl Signer {
    /// Add the credential to one request of `body` to `url`. `SigV4` signs
    /// `base`, so the request must carry exactly those headers and bytes.
    pub(crate) async fn authorize(
        &self,
        request: RequestBuilder,
        url: &str,
        base: &[(&str, &str)],
        body: &[u8],
    ) -> Result<RequestBuilder, Failure> {
        match self {
            Self::Anonymous => Ok(request),
            Self::Bearer(token) => Ok(request.bearer_auth(token)),
            Self::SigV4(identity, service) => {
                let headers = identity
                    .headers(service, url, base, body)
                    .await
                    .map_err(|error| Failure::new(CODE_AUTHENTICATION, error.to_string()))?;
                Ok(headers.into_iter().fold(request, |request, (name, value)| {
                    request.header(name, value)
                }))
            }
        }
    }
}
