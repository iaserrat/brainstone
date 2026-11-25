use crate::{ConfigError, auth::Signer};
use agent::Egress;
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
    /// The route this client takes; kept so a rebuilt client keeps it.
    pub(crate) egress: Egress,
    /// The idle timeout this client was built with, for the same reason.
    idle: Duration,
}

impl Http {
    pub(crate) fn new(endpoint: &str, signer: Signer) -> Result<Self, ConfigError> {
        let endpoint =
            Url::parse(endpoint).map_err(|error| ConfigError::endpoint(error.to_string()))?;
        ConfigError::validate_endpoint(&endpoint)?;
        Ok(Self {
            client: client(IDLE_TIMEOUT, &Egress::Direct)?,
            endpoint,
            signer,
            headers: HeaderMap::new(),
            egress: Egress::Direct,
            idle: IDLE_TIMEOUT,
        })
    }

    pub(crate) fn idle_timeout(&mut self, timeout: Duration) -> Result<(), ConfigError> {
        self.client = client(timeout, &self.egress)?;
        self.idle = timeout;
        Ok(())
    }

    pub(crate) fn set_egress(&mut self, egress: &Egress) -> Result<(), ConfigError> {
        self.client = client(self.idle, egress)?;
        self.egress = egress.clone();
        Ok(())
    }
    pub(crate) fn redacted_endpoint(&self) -> Url {
        let mut endpoint = self.endpoint.clone();
        endpoint.set_query(None);
        endpoint.set_fragment(None);
        endpoint
    }
}

fn client(timeout: Duration, egress: &Egress) -> Result<Client, ConfigError> {
    let builder = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(timeout)
        .redirect(reqwest::redirect::Policy::none());
    let builder = match egress.proxy_url() {
        Some(url) => builder.proxy(proxy(url)?),
        None => builder,
    };
    builder.build().map_err(|error| ConfigError::client(&error))
}

/// Only an HTTP proxy is usable: the client is built without SOCKS support, and
/// reqwest refuses an unknown scheme on the first request instead of here.
fn proxy(url: &str) -> Result<reqwest::Proxy, ConfigError> {
    let parsed = Url::parse(url).map_err(|error| ConfigError::proxy(error.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ConfigError::proxy(format!(
            "{url} is not an http or https proxy URL"
        )));
    }
    reqwest::Proxy::all(parsed).map_err(|error| ConfigError::proxy(error.to_string()))
}
