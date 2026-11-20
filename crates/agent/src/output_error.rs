use std::{error, fmt};

/// An invalid output contract was rejected before model execution.
pub const CODE_INVALID_OUTPUT_SCHEMA: &str = "invalid_output_schema";
/// The model did not produce a complete JSON document satisfying its contract.
pub const CODE_STRUCTURED_OUTPUT_INVALID: &str = "structured_output_invalid";

/// Why a structured-output contract or the model's final document was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutputError {
    /// The schema name is not 1..=64 ASCII letters, digits, underscores or hyphens.
    InvalidName,
    /// The schema is invalid or refers to an unavailable external resource.
    InvalidSchema,
    /// The final text is not one complete JSON document.
    InvalidJson,
    /// The JSON document does not satisfy its output schema.
    SchemaMismatch,
}

impl OutputError {
    #[must_use]
    /// The stable failure code a caller branches on.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidName | Self::InvalidSchema => CODE_INVALID_OUTPUT_SCHEMA,
            Self::InvalidJson | Self::SchemaMismatch => CODE_STRUCTURED_OUTPUT_INVALID,
        }
    }
}

impl fmt::Display for OutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidName => "output schema name must contain 1..=64 ASCII letters, digits, underscores or hyphens",
            Self::InvalidSchema => "output schema is invalid or requires an external resource",
            Self::InvalidJson => "final output is not one complete JSON document",
            Self::SchemaMismatch => "final output does not satisfy its JSON schema",
        })
    }
}

impl error::Error for OutputError {}
