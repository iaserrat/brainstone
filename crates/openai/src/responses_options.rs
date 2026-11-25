use crate::{ConfigError, ResponsesModel, auth::Signer};
use reqwest::header::HeaderMap;
use std::time::Duration;

impl ResponsesModel {
    #[must_use]
    /// Adds query parameters the target requires.
    pub fn with_parameters(mut self, parameters: crate::RequestParameters) -> Self {
        self.parameters = parameters;
        self
    }

    /// Build a local or otherwise unauthenticated Responses target.
    ///
    /// # Errors
    /// Rejects invalid endpoints and empty model identifiers.
    pub fn unauthenticated(endpoint: &str, model: impl Into<String>) -> Result<Self, ConfigError> {
        Self::build(endpoint, &model.into(), Signer::Anonymous)
    }

    /// Add operator-owned endpoint headers. Header values are never included in Debug.
    #[must_use]
    pub fn with_headers(mut self, headers: HeaderMap) -> Self {
        self.http.headers = headers;
        self
    }

    /// Bound silence between response bytes, including local model cold starts.
    ///
    /// # Errors
    /// Reports HTTP client configuration failures.
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Result<Self, ConfigError> {
        self.http.idle_timeout(timeout)?;
        Ok(self)
    }

    /// Hand every request to one HTTP proxy instead of the host's own route.
    ///
    /// # Errors
    /// Reports an unusable proxy URL or HTTP client configuration failure.
    pub fn with_egress(mut self, egress: &agent::Egress) -> Result<Self, ConfigError> {
        self.http.set_egress(egress)?;
        Ok(self)
    }
}
