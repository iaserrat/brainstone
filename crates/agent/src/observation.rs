//! Attempt observations never participate in durable state or execution policy.
use std::future::Future;
use tracing::{Instrument as _, Span};

/// Fixed boundaries keep metric dimensions independent of agent/plugin names.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    /// A whole accepted operation.
    Operation,
    /// One provider call.
    Model,
    /// One tool call.
    Tool,
    /// Waiting for an MCP client.
    McpQueue,
    /// One MCP tool call.
    McpCall,
    /// One child agent run.
    Subagent,
    /// One reply delivery.
    Delivery,
    /// One provider connection or retry.
    Provider,
    /// One compaction.
    Compaction,
    /// One recovery pass.
    Recovery,
}

impl Kind {
    /// Stable exported boundary name, independent of agent configuration.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Operation => "operation",
            Self::Model => "model",
            Self::Tool => "tool",
            Self::McpQueue => "mcp.queue",
            Self::McpCall => "mcp.call",
            Self::Subagent => "subagent",
            Self::Delivery => "delivery",
            Self::Provider => "provider",
            Self::Compaction => "compaction",
            Self::Recovery => "recovery",
        }
    }
}

/// A fresh attempt; identifiers are trace/log attributes, never metric labels.
#[must_use]
pub fn span(kind: Kind) -> Span {
    tracing::info_span!(target: "brainstone::telemetry", "work", kind = kind.name(),
        outcome = tracing::field::Empty, otel.status_code = tracing::field::Empty,
        operation_id = tracing::field::Empty, session_id = tracing::field::Empty,
        lane_id = tracing::field::Empty, parent_operation_id = tracing::field::Empty,
        child_operation_id = tracing::field::Empty, attempt = tracing::field::Empty)
}

/// Observe a future through its actual result. Dropping it leaves it abandoned.
/// `outcome` returns one of `ok`, `failed`, `cancelled`, `retry`, or `lease_lost`.
pub async fn run<F: Future>(
    span: Span,
    work: F,
    outcome: impl FnOnce(&F::Output) -> &'static str,
) -> F::Output {
    let result = work.instrument(span.clone()).await;
    let outcome = outcome(&result);
    span.record("outcome", outcome);
    if outcome == "failed" {
        span.record("otel.status_code", "ERROR");
    }
    span.in_scope(|| tracing::info!(name: "brainstone.work.finished", target: "brainstone::telemetry", outcome, "work finished"));
    result
}

/// Classify the semantic model response, including provider failures in `Ok`.
#[must_use]
pub const fn model_outcome(response: &crate::ModelResponse) -> &'static str {
    match response.stop {
        crate::StopReason::Error { .. } => "failed",
        _ => "ok",
    }
}

/// Classify an effect result without exporting its error or contents.
#[must_use]
pub const fn result_outcome<T, E>(result: &Result<T, E>) -> &'static str {
    if result.is_ok() { "ok" } else { "failed" }
}

#[path = "observation_model.rs"]
mod model;
pub use model::{generate, generate_stream};
