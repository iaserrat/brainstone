use agent::{ModelResponse, StopReason};

/// The provider answered with a body this adapter cannot read.
pub const INVALID_RESPONSE: &str = "invalid_response";
/// A tool definition was rejected before the request was sent.
pub const INVALID_TOOL: &str = "invalid_tool_definition";
/// The connection failed or was cut mid-response.
pub const TRANSPORT_ERROR: &str = "transport_error";
/// The request exceeded its deadline.
pub const REQUEST_TIMEOUT: &str = "request_timeout";
/// The credentials were rejected.
pub const AUTHENTICATION_FAILED: &str = "authentication_failed";
/// The account cannot be billed for the request.
pub const BILLING_ERROR: &str = "billing_error";
/// The credentials are valid but not allowed to do this.
pub const PERMISSION_DENIED: &str = "permission_denied";
/// The model or endpoint does not exist.
pub const NOT_FOUND: &str = "model_or_endpoint_not_found";
/// The request conflicts with the resource's current state.
pub const CONFLICT: &str = "request_conflict";
/// The provider asked the caller to slow down.
pub const RATE_LIMITED: &str = "rate_limited";
/// The provider rejected the request as malformed.
pub const INVALID_REQUEST: &str = "invalid_request";
/// The provider could not be reached.
pub const PROVIDER_UNAVAILABLE: &str = "provider_unavailable";
/// The provider failed while handling the request.
pub const PROVIDER_FAILED: &str = "provider_failed";
/// The response carried an HTTP status with no specific meaning here.
pub const HTTP_ERROR: &str = "http_error";
/// The model declined the request.
pub const REFUSED: &str = "refused";

/// Anthropic and Bedrock reject an overlong prompt as an ordinary invalid
/// request; these phrases in the message are the only thing telling it apart.
const OVERLONG_MARKERS: [&str; 3] = [
    "prompt is too long",
    "input is too long",
    "exceed context limit",
];

pub(crate) struct Failure {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

impl Failure {
    /// Every rejection passes through here, whichever path it arrived by, so
    /// an overlong prompt gets [`agent::CODE_CONTEXT_WINDOW_EXCEEDED`] once.
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        let message = message.into();
        let code = if code == INVALID_REQUEST && overlong(&message) {
            agent::CODE_CONTEXT_WINDOW_EXCEEDED
        } else {
            code
        };
        Self { code, message }
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
            REQUEST_TIMEOUT
        } else {
            TRANSPORT_ERROR
        };
        Self::new(code, error.to_string())
    }
}

fn overlong(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    OVERLONG_MARKERS
        .iter()
        .any(|marker| message.contains(marker))
}
