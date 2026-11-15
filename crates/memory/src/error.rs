use std::fmt;

/// Expected memory failures, reported to the model as structured tool errors.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The tool arguments did not match the schema the model was shown.
    InvalidArguments {
        /// The parser's message, for the model to correct its call.
        message: String,
    },
    /// A memory tool was called outside a durable operation.
    DurableContextRequired,
    /// A user-scoped memory was used by an operation without an actor.
    ActorRequired,
    /// A user-scoped memory was used outside a conversation private to its actor.
    PrivateConversationRequired,
    /// The store failed; carries the runtime's own code.
    Runtime(runtime::Error),
}

impl Error {
    /// The stable code a consumer branches on. Codes are public API.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidArguments { .. } => "invalid_memory_arguments",
            Self::DurableContextRequired => "durable_context_required",
            Self::ActorRequired => "memory_actor_required",
            Self::PrivateConversationRequired => "memory_private_conversation_required",
            Self::Runtime(error) => error.code(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArguments { message } => {
                write!(formatter, "invalid memory arguments: {message}")
            }
            Self::DurableContextRequired => {
                formatter.write_str("memory tools require a durable operation context")
            }
            Self::ActorRequired => {
                formatter.write_str("user-scoped memory requires an operation with an actor")
            }
            Self::PrivateConversationRequired => {
                formatter.write_str("user memory requires a conversation private to its actor")
            }
            Self::Runtime(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {}
