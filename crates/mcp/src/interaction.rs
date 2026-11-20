use std::pin::Pin;

use rmcp::model::{ElicitRequestParams, ElicitResult, ElicitationAction};

/// Which server is asking, on whose behalf, and what it requested.
#[derive(Debug, Clone)]
pub struct InteractionContext {
    /// The server that raised the elicitation.
    pub server: String,
    /// The idempotency key of the tool call that triggered it.
    pub call_key: String,
    /// The protocol request id.
    pub request_id: String,
    /// The elicitation the server sent.
    pub request: ElicitRequestParams,
}

/// Why a surface refused or failed to answer an elicitation.
#[derive(Debug, Clone)]
pub struct InteractionError {
    /// The stable code the server sees.
    pub code: &'static str,
    /// The human-readable reason.
    pub message: String,
}

impl InteractionError {
    #[must_use]
    /// Builds an error with a stable code.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// The surface's answer to a server's mid-call elicitation.
pub trait Interaction: Send + Sync {
    /// Answers one elicitation, or refuses with a stable code.
    fn elicit<'a>(
        &'a self,
        context: InteractionContext,
    ) -> Pin<Box<dyn Future<Output = Result<ElicitResult, InteractionError>> + Send + 'a>>;
}

#[derive(Default)]
pub(crate) struct Decline;

impl Interaction for Decline {
    fn elicit<'a>(
        &'a self,
        _context: InteractionContext,
    ) -> Pin<Box<dyn Future<Output = Result<ElicitResult, InteractionError>> + Send + 'a>> {
        Box::pin(async { Ok(ElicitResult::new(ElicitationAction::Decline)) })
    }
}
