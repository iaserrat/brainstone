use std::{error, fmt};

/// Why operator configuration was rejected. Every variant reports
/// `invalid_configuration`; the message names the setting.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigError {
    /// A required setting is absent.
    Missing {
        /// The variable's name.
        name: &'static str,
    },
    /// A setting is present but not a value this runtime accepts.
    Invalid {
        /// The variable's name.
        name: &'static str,
    },
    /// A setting names a path that cannot be read.
    Unreadable {
        /// The variable's name.
        name: &'static str,
    },
    /// A setting failed with a reason of its own.
    Detailed {
        /// The variable's name.
        name: &'static str,
        /// The reason the operator needs to see.
        message: String,
    },
}

impl ConfigError {
    /// The single code every configuration failure reports.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        "invalid_configuration"
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { name } => write!(formatter, "missing {name}"),
            Self::Invalid { name } => write!(formatter, "invalid {name}"),
            Self::Unreadable { name } => write!(formatter, "cannot read {name}"),
            Self::Detailed { name, message } => write!(formatter, "invalid {name}: {message}"),
        }
    }
}

impl error::Error for ConfigError {}
