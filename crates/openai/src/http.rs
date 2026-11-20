use crate::{ConfigError, auth::Signer};
use reqwest::{Client, Url, header::HeaderMap};
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(not(test))]
pub(crate) const IDLE_TIMEOUT: Duration = Duration::from_mins(2);
#[cfg(test)]
pub(crate) const IDLE_TIMEOUT: Duration = Duration::from_millis(500);

/// Transport policy is shared by protocol adapters, including authentication,
/// redirect refusal, idle timeouts, and status-only retries.
#[derive(Clone)]
pub(crate) struct Http {
    pub(crate) client: Client,
    pub(crate) endpoint: Url,
    pub(crate) signer: Signer,
    pub(crate) headers: HeaderMap,
}

impl Http {
    pub(crate) fn new(endpoint: &str, signer: Signer) -> Result<Self, ConfigError> {
        let endpoint =
            Url::parse(endpoint).map_err(|error| ConfigError::endpoint(error.to_string()))?;
        ConfigError::validate_endpoint(&endpoint)?;
        Ok(Self {
            client: client(IDLE_TIMEOUT)?,
            endpoint,
            signer,
            headers: HeaderMap::new(),
        })
    }

    pub(crate) fn idle_timeout(&mut self, timeout: Duration) -> Result<(), ConfigError> {
        self.client = client(timeout)?;
        Ok(())
    }
    pub(crate) fn redacted_endpoint(&self) -> Url {
        let mut endpoint = self.endpoint.clone();
        endpoint.set_query(None);
        endpoint.set_fragment(None);
        endpoint
    }
}

fn client(timeout: Duration) -> Result<Client, ConfigError> {
    Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| ConfigError::client(&error))
}
