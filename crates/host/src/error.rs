use std::error;
use std::fmt;

/// Expected host failures. Each variant carries a stable code a caller branches
/// on without parsing the message.
#[derive(Debug)]
pub enum Error {
    /// A configuration value is outside the range the host accepts.
    InvalidInput {
        /// The rejected setting, by name.
        field: &'static str,
    },
    /// Admission is closed: the host is draining.
    ShuttingDown,
    /// A worker task ended without finishing its operation.
    WorkerFailed {
        /// The worker's failure text.
        message: String,
    },
    /// The surface's admission policy refused the actor.
    AdmissionDenied,
    /// The actor reached its daily operation limit.
    AdmissionQuotaExceeded,
    /// The model selector produced no usable model and effort.
    ModelSelectionFailed,
    /// The store failed; carries the runtime's own code.
    Runtime(runtime::Error),
}

impl Error {
    /// The stable code a caller branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "invalid_input",
            Self::ShuttingDown => "host_shutting_down",
            Self::WorkerFailed { .. } => "worker_failed",
            Self::AdmissionDenied => "admission_denied",
            Self::AdmissionQuotaExceeded => "admission_quota_exceeded",
            Self::ModelSelectionFailed => "model_selection_failed",
            Self::Runtime(error) => error.code(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { field } => write!(formatter, "invalid {field}"),
            Self::ShuttingDown => formatter.write_str("host is shutting down"),
            Self::WorkerFailed { message } => formatter.write_str(message),
            Self::AdmissionDenied => formatter.write_str("the actor is not admitted"),
            Self::AdmissionQuotaExceeded => {
                formatter.write_str("the actor reached its daily operation limit")
            }
            Self::ModelSelectionFailed => {
                formatter.write_str("the model selector returned no usable model and effort")
            }
            Self::Runtime(error) => error.fmt(formatter),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            Self::InvalidInput { .. }
            | Self::ShuttingDown
            | Self::WorkerFailed { .. }
            | Self::AdmissionDenied
            | Self::AdmissionQuotaExceeded
            | Self::ModelSelectionFailed => None,
        }
    }
}

impl From<runtime::Error> for Error {
    fn from(error: runtime::Error) -> Self {
        Self::Runtime(error)
    }
}
