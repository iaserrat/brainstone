use std::fmt;

use crate::Error;

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArguments { message } => formatter.write_str(message),
            Self::UnknownSkill { name } => write!(formatter, "no skill named {name}"),
            Self::ResourceIo { path, message } => {
                write!(formatter, "cannot read {}: {message}", path.display())
            }
            Self::InvalidResourcePath { path } => write!(formatter, "invalid resource path {path}"),
            Self::ResourceNotText { path } => {
                write!(formatter, "resource is not UTF-8 text: {}", path.display())
            }
            Self::ResourceTooLarge { path } => {
                write!(formatter, "resource exceeds size limit: {}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {}
