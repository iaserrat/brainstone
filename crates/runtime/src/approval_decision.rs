use crate::operator_db::{execute, rows};
use crate::{ApprovalRecord, ApprovalState, DurableStore, Error};
impl DurableStore {
    /// Record an immutable decision. Actor is audit attribution; database access is authority.
    /// # Errors
    /// Returns `invalid_input` for expired/inactive requests or conflicting decisions.
    pub async fn decide_approval(
        &self,
        id: &str,
        approved: bool,
        actor: &str,
    ) -> Result<ApprovalRecord, Error> {
        if actor.trim().is_empty() {
            return Err(Error::InvalidInput { field: "operator" });
        }
        let decision = if approved { "approved" } else { "denied" };
        execute!(
            self,
            "UPDATE tool_approvals SET decision=$2,actor=$3 WHERE id=$1 AND decision IS NULL \
            AND expires_at_unix_ms>@now AND EXISTS(SELECT 1 FROM operations o WHERE o.id=operation_id AND o.state='running')",
            id,
            decision,
            actor
        )?;
        let record = self.approval(id).await?.ok_or(Error::InvalidInput {
            field: "approval_id",
        })?;
        check(&record, approved, actor)?;
        Ok(record)
    }

    /// Up to 100 pending approval IDs and operation IDs, without arguments or reasoning.
    /// # Errors
    /// Returns persistence or decoding failures.
    pub async fn pending_approvals(&self) -> Result<Vec<(String, i64)>, Error> {
        rows!(
            self,
            "SELECT a.id,a.operation_id FROM tool_approvals a JOIN operations o ON o.id=a.operation_id \
            WHERE a.decision IS NULL AND a.expires_at_unix_ms>@now AND o.state='running' ORDER BY a.operation_id,a.id LIMIT 100",
            (String, i64),
            fetch_all
        )
    }
}

fn check(record: &ApprovalRecord, approved: bool, actor: &str) -> Result<(), Error> {
    let expected = if approved {
        ApprovalState::Approved
    } else {
        ApprovalState::Denied
    };
    if record.state != expected || record.actor.as_deref() != Some(actor) {
        return Err(Error::InvalidInput {
            field: "approval_decision_conflict",
        });
    }
    Ok(())
}
