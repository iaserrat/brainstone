//! Minimal autonomy policy tools for the durable runtime.

#![deny(missing_docs)]
mod cancel_tool;
mod list_tool;
mod registry;
mod schedule_args;
mod schedule_definition;
mod schedule_tool;
mod silence_tool;
mod tool_support;

pub use registry::registry;
pub use silence_tool::Silence;

/// The shortest `repeat_every_minutes` an operator allows when it sets none.
pub const DEFAULT_MIN_INTERVAL_MINUTES: u32 = 15;

/// The instruction the runtime sends when it compacts a lane: summarize for
/// continuation, with no attempt to continue the work.
pub const COMPACTION_PROMPT: &str = "Summarize this conversation for seamless continuation. \
Preserve the user's goals and instructions, durable constraints, decisions, completed work, \
tool and sub-agent results, errors, exact identifiers and paths, and pending next steps. \
Do not continue the task or address the user. Return only the summary.";
