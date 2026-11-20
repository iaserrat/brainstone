//! Stable failures for this example's native tools.
#[derive(Debug)]
pub(crate) enum Error {
    InvalidPath,
    Io(std::io::Error),
    FileTooLarge,
    InvalidUtf8,
    DirectoryTooLarge,
    FileConflict,
    NoChange,
    ChecksFailed(String),
    ChecksTimeout,
    ChecksOutputLimit,
    ChecksAlreadyPassed,
    ChecksOrder,
    ContextRequired,
    JournalFailed(String),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<Error> for brainstone::ToolError {
    fn from(error: Error) -> Self {
        Self::new(error.code(), error.message())
    }
}

impl Error {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidPath => "coding_invalid_path",
            Self::Io(_) => "coding_io_failed",
            Self::FileTooLarge => "coding_file_too_large",
            Self::InvalidUtf8 => "coding_invalid_utf8",
            Self::DirectoryTooLarge => "coding_directory_too_large",
            Self::FileConflict => "coding_file_conflict",
            Self::NoChange => "coding_no_change",
            Self::ChecksFailed(_) => "coding_checks_failed",
            Self::ChecksTimeout => "coding_checks_timeout",
            Self::ChecksOutputLimit => "coding_checks_output_limit",
            Self::ChecksAlreadyPassed => "coding_checks_already_passed",
            Self::ChecksOrder => "coding_checks_order",
            Self::ContextRequired => "coding_context_required",
            Self::JournalFailed(_) => "coding_journal_failed",
        }
    }

    fn message(self) -> String {
        match self {
            Self::InvalidPath => "Use a relative path without symlinks, hidden components, or target directories.".into(),
            Self::Io(error) => error.to_string(),
            Self::FileTooLarge => "File contents exceed 64 KiB.".into(),
            Self::InvalidUtf8 => "File contents must be UTF-8.".into(),
            Self::DirectoryTooLarge => "Directory listing exceeds 64 visible entries.".into(),
            Self::FileConflict => "Current contents match neither expected nor replacement; no file was changed.".into(),
            Self::NoChange => "Expected contents and replacement must differ.".into(),
            Self::ChecksFailed(output) => output,
            Self::ChecksTimeout => "Cargo checks exceeded the 30-second deadline.".into(),
            Self::ChecksOutputLimit => "Cargo output exceeded 100 KiB.".into(),
            Self::ChecksAlreadyPassed => "This operation already committed successful checks; start a new operation to edit again.".into(),
            Self::ChecksOrder => "RunChecks must run in a separate assistant turn after the final edit.".into(),
            Self::ContextRequired => "A durable operation context is required.".into(),
            Self::JournalFailed(message) => message,
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
