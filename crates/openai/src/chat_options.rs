use crate::{ChatCompletionsModel, ConfigError};
use reqwest::header::HeaderMap;
use std::time::Duration;

impl ChatCompletionsModel {
    #[must_use]
    /// Adds query parameters the target requires.
    pub fn with_parameters(mut self, parameters: crate::RequestParameters) -> Self {
        self.parameters = parameters;
        self
    }

    #[must_use]
    /// Sends these headers with every request, such as a proxy's identity header.
    pub fn with_headers(mut self, headers: HeaderMap) -> Self {
        self.http.headers = headers;
        self
    }

    /// # Errors
    /// Reports HTTP client configuration failures.
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Result<Self, ConfigError> {
        self.http.idle_timeout(timeout)?;
        Ok(self)
    }
}
