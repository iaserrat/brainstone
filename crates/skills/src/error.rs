use std::path::PathBuf;

/// Expected skill failures, reported to the model as structured tool errors.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The tool arguments did not match the schema the model was shown.
    InvalidArguments {
        /// The parser's message, for the model to correct its call.
        message: String,
    },
    /// No skill loaded under that name.
    UnknownSkill {
        /// The requested skill name.
        name: String,
    },
    /// A skill resource could not be read.
    ResourceIo {
        /// The resource path.
        path: PathBuf,
        /// The I/O error.
        message: String,
    },
    /// The resource path escapes the skill directory.
    InvalidResourcePath {
        /// The rejected path.
        path: String,
    },
    /// The resource is not UTF-8 text.
    ResourceNotText {
        /// The resource path.
        path: PathBuf,
    },
    /// The resource exceeds the caller's size cap.
    ResourceTooLarge {
        /// The resource path.
        path: PathBuf,
    },
}
