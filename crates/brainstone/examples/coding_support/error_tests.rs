//! Every expected tool failure exposes a stable branch code.
use super::Error;

#[test]
fn coding_errors_keep_documented_codes() {
    for (error, code) in [
        (Error::InvalidPath, "coding_invalid_path"),
        (
            Error::Io(std::io::Error::other("fixture")),
            "coding_io_failed",
        ),
        (Error::FileTooLarge, "coding_file_too_large"),
        (Error::InvalidUtf8, "coding_invalid_utf8"),
        (Error::DirectoryTooLarge, "coding_directory_too_large"),
        (Error::FileConflict, "coding_file_conflict"),
        (Error::NoChange, "coding_no_change"),
        (
            Error::ChecksFailed("fixture".into()),
            "coding_checks_failed",
        ),
        (Error::ChecksTimeout, "coding_checks_timeout"),
        (Error::ChecksOutputLimit, "coding_checks_output_limit"),
        (Error::ChecksAlreadyPassed, "coding_checks_already_passed"),
        (Error::ChecksOrder, "coding_checks_order"),
        (Error::ContextRequired, "coding_context_required"),
        (
            Error::JournalFailed("fixture".into()),
            "coding_journal_failed",
        ),
    ] {
        assert_eq!(brainstone::ToolError::from(error).code, code);
    }
}
