use std::{error, fmt};

use crate::Error;

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } | Self::InvalidFrontmatter { path, message } => {
                write!(formatter, "{}: {message}", path.display())
            }
            Self::InvalidDefinition {
                path,
                field,
                message,
            } => {
                write!(formatter, "{}: invalid {field}: {message}", path.display())
            }
            Self::UnsupportedField { path, field } => {
                write!(formatter, "{}: unsupported field {field}", path.display())
            }
            other => display_other(other, formatter),
        }
    }
}

#[allow(clippy::too_many_lines, reason = "flat dispatch: one arm per variant")]
fn display_other(error: &Error, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match error {
        Error::DuplicateName { name } => write!(formatter, "duplicate agent name {name}"),
        Error::InvalidArguments { message }
        | Error::InvalidProfile { message }
        | Error::Encode { message } => formatter.write_str(message),
        Error::DurableContextRequired => formatter.write_str("Agent requires a durable operation"),
        Error::UnknownAgent { name } => write!(formatter, "unknown agent {name}"),
        Error::AgentDenied { name } => write!(formatter, "agent {name} is not allowed"),
        Error::DepthExceeded => formatter.write_str("agent depth limit reached"),
        Error::AgentLost => formatter.write_str("agent operation disappeared"),
        Error::Runtime(error) => write!(formatter, "{error}"),
        Error::UnknownMcpServer { name } => write!(formatter, "unknown MCP server {name}"),
        Error::NoTools { name } => write!(formatter, "agent {name} resolves to no tools"),
        Error::UnknownTool { agent, tool } => {
            write!(formatter, "agent {agent} names unknown tool {tool}")
        }
        Error::Io { .. }
        | Error::InvalidFrontmatter { .. }
        | Error::InvalidDefinition { .. }
        | Error::UnsupportedField { .. } => {
            unreachable!("path errors handled by Display")
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            _ => None,
        }
    }
}
