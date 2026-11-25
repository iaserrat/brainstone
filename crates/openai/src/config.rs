use std::fmt;

use crate::auth::Signer;

/// Invalid provider configuration, rejected before any request is made.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigError {
    /// The endpoint URL is not a usable API endpoint.
    InvalidEndpoint {
        /// The parser's message.
        message: String,
    },
    /// The HTTP client could not be built.
    ClientBuild {
        /// The TLS or runtime error.
        message: String,
    },
    /// The proxy this client was told to use is not a usable proxy URL.
    InvalidProxy {
        /// The parser's message.
        message: String,
    },
    /// The model id is blank.
    EmptyModel,
    /// The bearer token is blank.
    EmptyBearer,
    /// The AWS region is blank.
    EmptyRegion,
    /// A required environment variable is unset.
    MissingEnvironment {
        /// The variable's name.
        name: &'static str,
        /// What the variable must contain.
        message: String,
    },
}

impl ConfigError {
    /// The stable code a caller branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidEndpoint { .. } => "invalid_endpoint",
            Self::ClientBuild { .. } => "client_build_failed",
            Self::InvalidProxy { .. } => "invalid_proxy",
            Self::EmptyModel => "empty_model",
            Self::EmptyBearer => "empty_bearer",
            Self::EmptyRegion => "empty_region",
            Self::MissingEnvironment { .. } => "missing_environment",
        }
    }

    pub(crate) fn endpoint(message: String) -> Self {
        Self::InvalidEndpoint { message }
    }

    pub(crate) fn client(error: &reqwest::Error) -> Self {
        Self::ClientBuild {
            message: error.to_string(),
        }
    }

    pub(crate) fn proxy(message: String) -> Self {
        Self::InvalidProxy { message }
    }

    pub(crate) fn environment(name: &'static str, error: &std::env::VarError) -> Self {
        Self::MissingEnvironment {
            name,
            message: error.to_string(),
        }
    }

    pub(crate) fn validate(model: &str, signer: &Signer) -> Result<(), Self> {
        if model.trim().is_empty() {
            return Err(Self::EmptyModel);
        }
        if matches!(signer, Signer::Bearer(token) if token.trim().is_empty()) {
            return Err(Self::EmptyBearer);
        }
        Ok(())
    }

    pub(crate) fn validate_endpoint(endpoint: &reqwest::Url) -> Result<(), Self> {
        let http = matches!(endpoint.scheme(), "http" | "https");
        let no_credentials = endpoint.username().is_empty() && endpoint.password().is_none();
        if http && endpoint.host().is_some() && no_credentials {
            return Ok(());
        }
        Err(Self::InvalidEndpoint {
            message: "endpoint must be an absolute HTTP(S) URL without embedded credentials"
                .to_owned(),
        })
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEndpoint { message } => write!(formatter, "invalid endpoint: {message}"),
            Self::ClientBuild { message } => write!(formatter, "client build failed: {message}"),
            Self::InvalidProxy { message } => write!(formatter, "invalid proxy: {message}"),
            Self::EmptyModel => formatter.write_str("model must not be empty"),
            Self::EmptyBearer => formatter.write_str("bearer token must not be empty"),
            Self::EmptyRegion => formatter.write_str("AWS region is empty"),
            Self::MissingEnvironment { name, message } => {
                write!(formatter, "cannot read {name}: {message}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}
