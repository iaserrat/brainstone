//! Native Anthropic Messages API implementation of [`agent::Model`].

#![deny(missing_docs)]
mod auth;
mod config;
mod content_block;
mod content_output;
mod decode;
mod effort;
mod endpoint;
mod failure;
mod history;
mod http_status;
mod model;
mod model_build;
#[cfg(test)]
mod model_build_tests;
mod request;
mod runtime;
mod runtime_error;
mod sse;
mod stream;
mod stream_accumulate;
mod stream_event;
mod stream_flush;
mod stream_state;
mod tool_def;
mod transport;

pub use auth::Auth;
pub use config::ConfigError;
pub use effort::Effort;
pub use endpoint::Endpoint;
pub use failure::{
    AUTHENTICATION_FAILED, BILLING_ERROR, CONFLICT, HTTP_ERROR, INVALID_REQUEST, INVALID_RESPONSE,
    INVALID_TOOL, NOT_FOUND, PERMISSION_DENIED, PROVIDER_FAILED, PROVIDER_UNAVAILABLE,
    RATE_LIMITED, REFUSED, REQUEST_TIMEOUT, TRANSPORT_ERROR,
};
pub use model::AnthropicModel;
pub use sigv4::SigV4;

/// The provider name that selects the Anthropic Messages adapter.
pub const ANTHROPIC_PROVIDER: &str = "anthropic-messages";
