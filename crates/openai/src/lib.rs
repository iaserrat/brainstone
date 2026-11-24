//! A production [`agent::Model`] implementation for the `OpenAI` Responses API.

#![deny(missing_docs)]
mod auth;
mod bedrock;
mod config;
mod effort;
mod failure;
mod history;
mod output;
mod output_format;
mod request;
mod response;
mod response_error;
mod responses_call;
mod responses_debug;
mod responses_model;
#[cfg(test)]
mod responses_model_tests;
mod responses_stream;
mod sse;
mod stream;
mod stream_event;
mod stream_state;
mod tool_def;
mod transport;
mod transport_status;

pub use crate::auth::Auth;
pub use crate::config::ConfigError;
pub use crate::effort::Effort;
pub use crate::failure::{
    CODE_AUTHENTICATION, CODE_CONFLICT, CODE_CONTENT_FILTERED, CODE_FOREIGN_REASONING, CODE_HTTP,
    CODE_INVALID_REQUEST, CODE_INVALID_RESPONSE, CODE_INVALID_TOOL, CODE_NOT_FOUND,
    CODE_PERMISSION, CODE_PROVIDER_FAILED, CODE_PROVIDER_UNAVAILABLE, CODE_RATE_LIMITED,
    CODE_REFUSED, CODE_REQUEST_CANCELLED, CODE_TIMEOUT, CODE_TRANSPORT,
};
pub use crate::responses_model::ResponsesModel;
pub use sigv4::SigV4;

/// `OpenAI`'s first-party Responses endpoint.
pub const OPENAI_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";

/// Provider tag stored with opaque Responses reasoning items.
pub const OPENAI_RESPONSES_PROVIDER: &str = "openai-responses";

mod http;
mod responses_options;

mod chat_call;
mod chat_history;
mod chat_model;
mod chat_options;
mod chat_parts;
mod chat_reasoning;
mod chat_request;
mod chat_response;
mod chat_stream;
mod chat_stream_merge;
mod chat_stream_output;
mod chat_stream_state;
mod chat_tools;
mod chat_usage;
mod model_catalog;
pub use chat_model::ChatCompletionsModel;

/// Opaque Chat Completions continuation fields, stored without translation.
pub const CHAT_COMPLETIONS_PROVIDER: &str = "openai-chat-completions";

mod request_parameters;
pub use request_parameters::{ParameterError, RequestParameters};

#[cfg(test)]
mod output_schema_tests;
