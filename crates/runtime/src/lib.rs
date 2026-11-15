//! Durable sessions, lanes, and accepted operations for the agent loop.

#![deny(missing_docs)]
mod address;
mod admission_quota;
mod admitted;
mod approval;
mod approval_decision;
mod approval_record;
mod approval_store;
mod claim;
mod compaction;
mod compaction_policy;
mod config;
mod delivery;
mod delivery_target;
mod drive;
mod error;
mod event_sink;
mod execution;
mod exports;
mod interaction;
mod lanes;
mod memories;
mod operation;
mod operator;
mod operator_db;
mod operator_journal;
mod prompt;
mod prompt_access;
mod prompt_content;
mod runtime;
mod schedule;
mod schedule_destination;
mod schedule_failure;
mod schedule_owner;
mod schedule_repeat;
mod schedule_store;
mod session_delete;
mod session_delete_sql;
mod session_delete_store;
mod steering;
mod store;
mod store_error;
mod stored_media;
mod subagent;
mod subagent_config;
mod subagent_request;
mod tool_policy;

// Declaration lists, each documented in its own file.
include!("memory_modules.rs");
include!("postgres_modules.rs");
include!("sqlite_modules.rs");
include!("durable_modules.rs");
#[cfg(test)]
mod catalog_tests;
#[cfg(test)]
mod tests;
pub use approval::{ApprovalRecord, ApprovalRequest, ApprovalState};
/// The public API, listed one item at a time in `exports.rs`. The glob keeps
/// this file inside the charter's line budget; add items there, never here.
pub use exports::*;
pub use operator::OperationInspection;
pub use tool_policy::ToolPolicy;

mod stored_images;
mod stored_media_row;
mod stored_operation;

#[cfg(test)]
mod context_window_tests;
