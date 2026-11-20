//! Anthropic-compatible durable subagent definitions and controls.

#![deny(missing_docs)]
mod catalog;
mod control;
mod definition;
mod definition_access;
mod definition_validate;
mod error;
mod error_code;
mod error_display;
mod frontmatter;
mod message_tool;
mod output_tool;
mod output_wait;
mod raw;
mod selection;
mod spawn_args;
mod spawn_authorize;
mod spawn_config;
mod spawn_execute;
mod spawn_tool;
mod spawn_wait;
mod stop_tool;

pub use catalog::Catalog;
pub use control::Control;
pub use definition::Definition;
pub use error::Error;
pub use selection::{Effort, Model};
use spawn_tool::AgentTool;
