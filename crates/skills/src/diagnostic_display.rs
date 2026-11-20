use std::fmt;

use crate::Diagnostic;

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } | Self::InvalidFrontmatter { path, message } => {
                write!(formatter, "{}: {message}", path.display())
            }
            Self::NameMismatch {
                path,
                directory,
                name,
            } => write!(
                formatter,
                "{}: directory `{directory}` differs from name `{name}`",
                path.display()
            ),
            Self::NameDefaulted { path, name } => write!(
                formatter,
                "{}: missing name defaulted to `{name}`",
                path.display()
            ),
            Self::ScanLimit { path, maximum } => write!(
                formatter,
                "{}: skill directory limit of {maximum} exceeded",
                path.display()
            ),
        }
    }
}
