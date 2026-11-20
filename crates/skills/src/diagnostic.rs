use std::path::{Path, PathBuf};

/// One problem found while loading a skill directory. Each variant has a stable
/// code the operator branches on.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Diagnostic {
    /// The directory or its `SKILL.md` could not be read.
    Io {
        /// The path that failed.
        path: PathBuf,
        /// The I/O error.
        message: String,
    },
    /// `SKILL.md` has frontmatter this parser rejects.
    InvalidFrontmatter {
        /// The `SKILL.md` path.
        path: PathBuf,
        /// The parser's message.
        message: String,
    },
    /// The declared name differs from the directory name.
    NameMismatch {
        /// The `SKILL.md` path.
        path: PathBuf,
        /// The directory name.
        directory: String,
        /// The declared name.
        name: String,
    },
    /// Frontmatter declares no name, so the directory name was used.
    NameDefaulted {
        /// The `SKILL.md` path.
        path: PathBuf,
        /// The name taken from the directory.
        name: String,
    },
    /// The directory holds more entries than the loader scans.
    ScanLimit {
        /// The skill directory.
        path: PathBuf,
        /// The cap that was hit.
        maximum: usize,
    },
}

impl Diagnostic {
    pub(crate) fn io(path: &Path, error: &std::io::Error) -> Self {
        Self::Io {
            path: path.to_owned(),
            message: error.to_string(),
        }
    }

    pub(crate) fn invalid(path: &Path, message: impl Into<String>) -> Self {
        Self::InvalidFrontmatter {
            path: path.to_owned(),
            message: message.into(),
        }
    }

    pub(crate) fn mismatch(path: &Path, directory: &str, name: &str) -> Self {
        Self::NameMismatch {
            path: path.to_owned(),
            directory: directory.to_owned(),
            name: name.to_owned(),
        }
    }

    pub(crate) fn defaulted(path: &Path, name: &str) -> Self {
        Self::NameDefaulted {
            path: path.to_owned(),
            name: name.to_owned(),
        }
    }

    pub(crate) fn scan_limit(path: &Path, maximum: usize) -> Self {
        Self::ScanLimit {
            path: path.to_owned(),
            maximum,
        }
    }

    /// The stable code an operator's log line branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "skill_io",
            Self::InvalidFrontmatter { .. } => "invalid_skill_frontmatter",
            Self::NameMismatch { .. } => "skill_name_mismatch",
            Self::NameDefaulted { .. } => "skill_name_defaulted",
            Self::ScanLimit { .. } => "skill_scan_limit",
        }
    }
}
