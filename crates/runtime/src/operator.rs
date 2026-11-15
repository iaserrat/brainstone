use crate::operator_db::rows;
use crate::{DurableStore, Error, OperationId};
use serde::Serialize;
/// Metadata only: no prompts, arguments, outcomes, or reasoning.
#[derive(Debug, Serialize)]
pub struct OperationInspection {
    /// The operation id.
    pub id: i64,
    /// Its session.
    pub session_id: i64,
    /// Its lane.
    pub lane_id: i64,
    /// Its durable state, as text.
    pub state: String,
    /// The worker holding the lease, if any.
    pub lease_owner: Option<String>,
    /// Whether that lease is unexpired.
    pub lease_active: bool,
    /// Journal entries recorded.
    pub events: i64,
    /// Steering messages not yet consumed.
    pub pending_steering: i64,
    /// Approvals still unanswered.
    pub pending_approvals: i64,
}
type Row = (i64, i64, i64, String, Option<String>, bool, i64, i64, i64);
const SELECT: &str = "SELECT o.id,l.session_id,o.lane_id,o.state,s.lease_owner, \
    COALESCE(s.lease_expires_at>@lease_now,false),o.next_event_sequence, \
    (SELECT count(*) FROM operation_steering st WHERE st.operation_id=o.id AND NOT st.consumed), \
    (SELECT count(*) FROM tool_approvals a WHERE a.operation_id=o.id AND a.decision IS NULL AND a.expires_at_unix_ms>@now) \
    FROM operations o JOIN lanes l ON l.id=o.lane_id JOIN sessions s ON s.id=l.session_id";
impl DurableStore {
    /// Newest operations before an exclusive ID cursor. Limit must be 1..=100.
    /// # Errors
    /// Returns invalid input, storage failures or uninterpretable records.
    pub async fn operations(
        &self,
        before: Option<OperationId>,
        limit: u16,
    ) -> Result<Vec<OperationInspection>, Error> {
        if !(1..=100).contains(&limit) {
            return Err(Error::InvalidInput { field: "limit" });
        }
        let source = format!("{SELECT} WHERE o.id<$1 ORDER BY o.id DESC LIMIT $2");
        Ok(rows!(
            self,
            &source,
            Row,
            fetch_all,
            before.map_or(i64::MAX, OperationId::get),
            i64::from(limit)
        )?
        .into_iter()
        .map(inspect)
        .collect())
    }
    /// One statement reads lease facts using database time; never changes ownership.
    /// # Errors
    /// Returns storage failures; missing IDs return `None`.
    pub async fn inspect(&self, id: OperationId) -> Result<Option<OperationInspection>, Error> {
        let source = format!("{SELECT} WHERE o.id=$1");
        Ok(rows!(self, &source, Row, fetch_optional, id.get())?.map(inspect))
    }
}
fn inspect(row: Row) -> OperationInspection {
    OperationInspection {
        id: row.0,
        session_id: row.1,
        lane_id: row.2,
        state: row.3,
        lease_owner: row.4,
        lease_active: row.5,
        events: row.6,
        pending_steering: row.7,
        pending_approvals: row.8,
    }
}
