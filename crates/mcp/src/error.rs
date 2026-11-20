use std::fmt;

#[derive(Debug)]
/// An MCP failure reported to the model as a stable code plus a message.
pub struct Error {
    code: &'static str,
    message: String,
}

impl Error {
    pub(crate) fn new(code: &'static str, error: impl fmt::Display) -> Self {
        Self {
            code,
            message: error.to_string(),
        }
    }

    #[must_use]
    /// The stable code a caller branches on.
    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}
