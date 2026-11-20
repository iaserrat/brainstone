pub(crate) const ROOT: &str = "SELECT id, EXISTS(SELECT 1 FROM subagent_runs WHERE child_session_id=s.id) \
    FROM sessions s WHERE surface=$1 AND external_id=$2 @lock";
pub(crate) const TREE: &str = "SELECT id, COALESCE(lease_expires_at>@now,false) FROM sessions \
    WHERE id=$1 OR id IN (SELECT child_session_id FROM subagent_runs WHERE root_session_id=$1) \
    ORDER BY id @lock";
pub(crate) const DELIVERIES: &str = "SELECT COALESCE(lease_expires_at>@now,false) \
    FROM operation_deliveries WHERE operation_id IN \
    (SELECT id FROM operations WHERE lane_id IN \
    (SELECT id FROM lanes WHERE session_id IN (@sessions))) @lock";

const SESSIONS: &str = "SELECT id FROM sessions WHERE id IN (@sessions)";

pub(crate) fn statements() -> Vec<String> {
    let lanes = format!("SELECT id FROM lanes WHERE session_id IN ({SESSIONS})");
    let operations = format!("SELECT id FROM operations WHERE lane_id IN ({lanes})");
    let steering = format!("SELECT id FROM operation_steering WHERE lane_id IN ({lanes})");
    let mut statements = dependents(&operations, &steering);
    statements.extend([
        format!("DELETE FROM subagent_runs WHERE root_session_id IN ({SESSIONS})"),
        format!("DELETE FROM tool_media WHERE root_session_id IN ({SESSIONS})"),
        format!(
            "UPDATE operations SET schedule_id=NULL WHERE lane_id IN ({lanes}) OR \
            schedule_id IN (SELECT id FROM scheduled_work WHERE lane_id IN ({lanes}))"
        ),
        format!(
            "UPDATE scheduled_work SET released_operation_id=NULL \
            WHERE released_operation_id IN ({operations})"
        ),
        format!("DELETE FROM scheduled_work WHERE lane_id IN ({lanes})"),
        format!("DELETE FROM operation_steering WHERE lane_id IN ({lanes})"),
        format!("DELETE FROM lane_compactions WHERE lane_id IN ({lanes})"),
        format!("DELETE FROM lane_follow_activations WHERE lane_id IN ({lanes})"),
        format!("DELETE FROM lane_unfollow_calls WHERE lane_id IN ({lanes})"),
        format!("DELETE FROM operations WHERE lane_id IN ({lanes})"),
        format!("DELETE FROM lanes WHERE session_id IN ({SESSIONS})"),
        "DELETE FROM sessions WHERE id IN (@sessions)".to_owned(),
    ]);
    statements
}

fn dependents(operations: &str, steering: &str) -> Vec<String> {
    let mut statements = [
        "operation_events",
        "operation_deliveries",
        "interactions",
        "operation_images",
        "tool_approvals",
    ]
    .map(|table| format!("DELETE FROM {table} WHERE operation_id IN ({operations})"))
    .to_vec();
    statements.extend(
        ["operation_steering_images"]
            .map(|table| format!("DELETE FROM {table} WHERE steering_id IN ({steering})")),
    );
    statements
}
