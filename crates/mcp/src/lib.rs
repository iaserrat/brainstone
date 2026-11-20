//! Official Model Context Protocol client and agent-tool adapter.

#![deny(missing_docs)]
mod catalog;
#[cfg(test)]
mod catalog_tests;
mod client;
mod client_peer;
mod config;
mod config_binding;
mod config_build;
mod config_debug;
mod config_env;
mod config_raw;
mod config_validate;
mod connect;
mod connect_transport;
mod error;
mod handler;
mod handler_interaction;
mod handler_support;
mod input;
mod interaction;
mod session;
mod set;
mod timeout;
mod tool;
mod tool_call;
mod tool_content;
mod tool_content_file;
mod tool_content_image;
mod tool_content_note;
#[cfg(test)]
mod tool_content_tests;
mod tool_execution;
mod tool_lifecycle;
mod tool_protocol;
mod tool_task;

pub use crate::client::Client;
pub use crate::config::{Config, Server, Transport};
pub use crate::error::Error;
pub use crate::interaction::{Interaction, InteractionContext, InteractionError};
pub use crate::set::ClientSet;
pub use rmcp;

#[cfg(test)]
mod config_tests;
#[cfg(test)]
mod connect_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod timeout_tests;

mod request_meta;
