use crate::{ApprovalRecord, ApprovalRequest, ApprovalState, Error};
pub(crate) type Row = (String, String, Option<String>, Option<String>, i64, i64);
pub(crate) fn decode(row: Row) -> Result<ApprovalRecord, Error> {
    let request: ApprovalRequest = serde_json::from_str(&row.1).map_err(Error::corrupt)?;
    if !request.known_version() || request.id() != row.0 {
        return Err(Error::CorruptRecord {
            message: "approval digest mismatch".into(),
        });
    }
    let state = decision(row.2.as_deref())?;
    Ok(ApprovalRecord {
        id: row.0,
        request,
        state: if row.4 <= row.5 {
            ApprovalState::Expired
        } else {
            state
        },
        actor: row.3,
        expires_at_unix_ms: row.4,
    })
}

fn decision(value: Option<&str>) -> Result<ApprovalState, Error> {
    Ok(match value {
        None => ApprovalState::Pending,
        Some("approved") => ApprovalState::Approved,
        Some("denied") => ApprovalState::Denied,
        _ => {
            return Err(Error::CorruptRecord {
                message: "unknown approval decision".into(),
            });
        }
    })
}

#[cfg(test)]
#[path = "approval_record_tests.rs"]
mod tests;
