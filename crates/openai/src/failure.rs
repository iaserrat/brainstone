use agent::{ModelResponse, StopReason};

/// Reasoning arrived from a provider that did not produce the turn.
pub const CODE_FOREIGN_REASONING: &str = "foreign_reasoning";
/// A tool definition was rejected before the request was sent.
pub const CODE_INVALID_TOOL: &str = "invalid_tool_definition";
/// The provider answered with a body this adapter cannot read.
pub const CODE_INVALID_RESPONSE: &str = "invalid_response";
/// The model declined the request.
pub const CODE_REFUSED: &str = "refused";
/// The provider's content filter blocked the output.
pub const CODE_CONTENT_FILTERED: &str = "content_filtered";
/// The provider failed while handling the request.
pub const CODE_PROVIDER_FAILED: &str = "provider_failed";
/// The request was cancelled before it completed.
pub const CODE_REQUEST_CANCELLED: &str = "request_cancelled";
/// The request exceeded its deadline.
pub const CODE_TIMEOUT: &str = "request_timeout";
/// The connection failed or was cut mid-response.
pub const CODE_TRANSPORT: &str = "transport_error";
/// The credentials were rejected.
pub const CODE_AUTHENTICATION: &str = "authentication_failed";
/// The credentials are valid but not allowed to do this.
pub const CODE_PERMISSION: &str = "permission_denied";
/// The model or endpoint does not exist.
pub const CODE_NOT_FOUND: &str = "model_or_endpoint_not_found";
/// The provider asked the caller to slow down.
pub const CODE_RATE_LIMITED: &str = "rate_limited";
/// The provider rejected the request as malformed.
pub const CODE_INVALID_REQUEST: &str = "invalid_request";
/// The provider could not be reached.
pub const CODE_PROVIDER_UNAVAILABLE: &str = "provider_unavailable";
/// The response carried an HTTP status with no specific meaning here.
pub const CODE_HTTP: &str = "http_error";
/// The request conflicts with the resource's current state.
pub const CODE_CONFLICT: &str = "request_conflict";

/// The provider names an overlong input by this error code on every path.
const CONTEXT_LENGTH_EXCEEDED: &str = "context_length_exceeded";

/// `fallback`, unless the provider's own error `code` says the input overflowed the
/// context window: the one rejection a caller can repair by compacting.
pub(crate) fn classified(code: Option<&str>, fallback: &'static str) -> &'static str {
    if code == Some(CONTEXT_LENGTH_EXCEEDED) {
        agent::CODE_CONTEXT_WINDOW_EXCEEDED
    } else {
        fallback
    }
}

pub(crate) struct Failure {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

impl Failure {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn response(self) -> ModelResponse {
        ModelResponse {
            model: None,
            reasoning: Vec::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            stop: StopReason::Error {
                code: self.code.to_owned(),
                message: self.message,
            },
        }
    }
}

impl From<reqwest::Error> for Failure {
    fn from(error: reqwest::Error) -> Self {
        let code = if error.is_timeout() {
            CODE_TIMEOUT
        } else {
            CODE_TRANSPORT
        };
        Self::new(code, error.without_url().to_string())
    }
}
