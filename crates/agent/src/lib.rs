//! The agent core loop: the data it reads, the seams it calls, and the driver.
//!
//! Durability is deliberately absent. The loop reads a [`Thread`], emits
//! [`Event`]s, and returns an [`Outcome`]; persisting those events and
//! rebuilding the thread on resume belongs to a caller. Keeping that seam
//! uncrossed is what lets one loop serve any surface and any provider.
//!
//! Retries are absent for the same reason. A [`Model`] implementation owns its
//! own backoff and timeouts and reports what it could not recover from as
//! [`StopReason::Error`], so the loop only ever sees settled responses.

#![deny(missing_docs)]
mod bytes64;
mod call;
mod event;
mod file;
mod image;
mod model;
mod next;
#[cfg(test)]
mod next_tests;
mod profile;
mod profile_error;
mod progress;
#[cfg(test)]
mod progress_tests;
mod provider;
mod registry;
mod registry_select;
#[cfg(test)]
mod registry_tests;
mod run;
#[cfg(test)]
mod run_tests;
mod step;
mod thread;
#[cfg(test)]
mod thread_replay_tests;
mod tool;
mod tool_bound;
mod tool_output;
mod tool_source;
mod turn_limit;
mod user_images;

pub use crate::call::{execute_tool, execute_tool_with_context, execute_tool_with_media, refused};
pub use crate::event::{Event, Sink};
pub use crate::file::{File, FileError, MAX_FILE_NAME_BYTES};
pub use crate::image::{Image, ImageError, ImageMediaType};
pub use crate::model::{
    CODE_CONTEXT_WINDOW_EXCEEDED, ModelResponse, Reasoning, StopReason, ToolCall,
};
pub use crate::next::next;
pub use crate::profile::{Effort, ExecutionProfile, ProfileError};
pub use crate::progress::Progress;
pub use crate::provider::Model;
pub use crate::registry::Registry;
pub use crate::run::run;
pub use crate::step::{
    CODE_STEP_BUDGET, CODE_TRUNCATED_CALL, CODE_UNANSWERED_CALL, CODE_UNKNOWN_TOOL, MAX_STEPS,
    Outcome, SILENCE_TOOL, Step,
};
pub use crate::thread::{Message, Thread};
pub use crate::tool::{Tool, ToolContext, ToolError, ToolRequest, ToolResult};
pub use crate::tool_bound::{CODE_TOOL_RESULT_TRUNCATED, MAX_TOOL_RESULT_BYTES};
pub use crate::tool_output::ToolOutput;
pub use crate::tool_source::ToolSource;
pub use crate::turn_limit::TurnLimit;
pub use crate::user_images::UserImages;

mod correction;
pub use correction::output_feedback;

mod access;
mod identity;
pub use access::{AccessError, AccessPolicy};
pub use identity::ExecutionIdentity;

/// Content-free observations of actual execution attempts.
pub mod observation;

mod capability_error;
mod capability_model;
mod model_capabilities;
pub use capability_error::CapabilityError;
pub use capability_model::CapabilityModel;
pub use model_capabilities::ModelCapabilities;
