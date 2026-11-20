use std::{error, fmt};

/// Why a model configuration was rejected. Each variant carries a stable code.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigError {
    /// The endpoint URL is not a usable Messages endpoint.
    InvalidEndpoint {
        /// The parser's message.
        message: String,
    },
    /// The model id is blank.
    EmptyModel,
    /// The API key is blank.
    EmptyApiKey,
    /// The AWS region is blank.
    EmptyRegion,
    /// `max_tokens` is zero.
    InvalidMaxTokens,
    /// A required environment variable is unset.
    MissingEnvironment {
        /// The variable's name.
        name: &'static str,
    },
    /// The HTTP client could not be built.
    ClientBuild {
        /// The TLS or runtime error.
        message: String,
    },
}

impl ConfigError {
    /// The stable code a caller branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidEndpoint { .. } => "invalid_endpoint",
            Self::EmptyModel => "empty_model",
            Self::EmptyApiKey => "empty_api_key",
            Self::EmptyRegion => "empty_region",
            Self::InvalidMaxTokens => "invalid_max_tokens",
            Self::MissingEnvironment { .. } => "missing_environment",
            Self::ClientBuild { .. } => "client_build_failed",
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEndpoint { message } | Self::ClientBuild { message } => {
                formatter.write_str(message)
            }
            Self::EmptyModel => formatter.write_str("model is empty"),
            Self::EmptyApiKey => formatter.write_str("API key is empty"),
            Self::EmptyRegion => formatter.write_str("AWS region is empty"),
            Self::InvalidMaxTokens => formatter.write_str("max_tokens must be positive"),
            Self::MissingEnvironment { name } => write!(formatter, "missing {name}"),
        }
    }
}

impl error::Error for ConfigError {}
