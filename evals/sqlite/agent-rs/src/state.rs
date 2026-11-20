//! Native state is reported without an evaluation-owned receipt cache.
use crate::protocol::emit;
use agent_rs::{OperationStatus, Phase, RunState, Settlement};
use serde_json::json;

pub fn emit_state(state: &RunState) {
    match &state.phase {
        Phase::Settled(Settlement::Completed(value)) => emit(&json!({"event":"terminal",
            "status":"completed", "value":value, "operation_id":state.id.to_string()})),
        Phase::Settled(settlement) => emit(&json!({"event":"terminal", "status":"failed",
            "code":settlement_code(settlement), "operation_id":state.id.to_string()})),
        _ => emit_pending(state),
    }
}

pub fn waiting(state: &RunState) -> bool {
    state.operations.iter().any(|operation| {
        matches!(
            operation.status,
            OperationStatus::Unknown { .. }
                | OperationStatus::Approval
                | OperationStatus::Waiting(_)
        )
    })
}

fn emit_pending(state: &RunState) {
    emit(
        &json!({"event":"state", "status":if waiting(state) {"waiting"} else {"pending"},
        "operation_id":state.id.to_string(), "code":if waiting(state) {"reconciliation_required"} else {"pending"}}),
    );
}

fn settlement_code(settlement: &Settlement) -> &'static str {
    match settlement {
        Settlement::Stopped(_) => "stopped",
        Settlement::Failed(_) => "run_failed",
        Settlement::Cancelled => "cancelled",
        Settlement::Completed(_) => "completed",
    }
}
