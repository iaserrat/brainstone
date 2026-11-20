use std::{error, fmt};

use crate::configured::ConfigError;

/// A failure composing or starting the engine.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Configuration was rejected before anything was built.
    Configuration(ConfigError),
    /// A component failed while the engine was composed.
    Component {
        /// The component's stable code.
        code: &'static str,
        /// What the component reported.
        message: String,
    },
}

impl Error {
    /// The stable code a caller branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Configuration(error) => error.code(),
            Self::Component { code, .. } => code,
        }
    }

    pub(crate) fn component(code: &'static str, error: impl fmt::Display) -> Self {
        Self::Component {
            code,
            message: error.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(error) => error.fmt(formatter),
            Self::Component { message, .. } => formatter.write_str(message),
        }
    }
}

impl error::Error for Error {}

impl From<ConfigError> for Error {
    fn from(error: ConfigError) -> Self {
        Self::Configuration(error)
    }
}
