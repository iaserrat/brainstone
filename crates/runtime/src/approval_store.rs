use crate::approval_record::{Row, decode};
use crate::operator_db::{execute, rows};
use crate::{ApprovalRecord, ApprovalRequest, ApprovalState, DurableStore, Error, Lease};
const COLUMNS: &str =
    "SELECT id,request,decision,actor,expires_at_unix_ms,@now FROM tool_approvals";
const TTL_MS: i64 = 900_000;

impl DurableStore {
    pub(crate) async fn authorize(
        &self,
        lease: &Lease,
        request: &ApprovalRequest,
    ) -> Result<ApprovalState, Error> {
        if request.operation != lease.operation_id {
            return Err(Error::LeaseLost);
        }
        fenced(self, lease).await?;
        let id = insert(self, lease, request).await?;
        let record = self.approval(&id).await?.ok_or(Error::LeaseLost)?;
        if record.request != *request {
            return Err(Error::CorruptRecord {
                message: "approval binding mismatch".into(),
            });
        }
        Ok(record.state)
    }

    /// Exact request contents for explicit operator review. Missing IDs return `None`.
    /// # Errors
    /// Returns persistence failures or rejects uninterpretable approval data.
    pub async fn approval(&self, id: &str) -> Result<Option<ApprovalRecord>, Error> {
        let source = format!("{COLUMNS} WHERE id=$1");
        rows!(self, &source, Row, fetch_optional, id)?
            .map(decode)
            .transpose()
    }
}

async fn insert(
    store: &DurableStore,
    lease: &Lease,
    request: &ApprovalRequest,
) -> Result<String, Error> {
    let body = serde_json::to_string(request).map_err(Error::corrupt)?;
    let id = request.id();
    execute!(
        store,
        "INSERT INTO tool_approvals(id,operation_id,request,expires_at_unix_ms) \
            SELECT $1,$2,$3,@now+$4 FROM sessions s JOIN lanes l ON l.session_id=s.id \
            JOIN operations o ON o.lane_id=l.id WHERE o.id=$2 AND s.id=$5 AND s.lease_owner=$6 \
            AND s.lease_token=$7 AND o.lease_token=$7 AND o.state='running' AND s.lease_expires_at>@lease_now \
            ON CONFLICT(id) DO NOTHING",
        &id,
        request.operation.get(),
        &body,
        TTL_MS,
        lease.session_id.get(),
        &lease.worker,
        lease.token
    )?;
    Ok(id)
}

async fn fenced(store: &DurableStore, lease: &Lease) -> Result<(), Error> {
    rows!(store,"SELECT o.id FROM operations o JOIN lanes l ON l.id=o.lane_id JOIN sessions s ON s.id=l.session_id WHERE o.id=$1 AND s.id=$2 AND s.lease_owner=$3 AND s.lease_token=$4 AND o.lease_token=$4 AND o.state='running' AND s.lease_expires_at>@lease_now",(i64,),fetch_optional,lease.operation_id.get(),lease.session_id.get(),&lease.worker,lease.token)?.ok_or(Error::LeaseLost)?;
    Ok(())
}
