use std::time::Duration;

use sqlx::SqlitePool;

use crate::{Error, Lease};

pub(crate) async fn heartbeat(pool: &SqlitePool, lease: &Lease, ttl: Duration) -> Result<(), Error> {
    let millis =
        i64::try_from(ttl.as_millis()).map_err(|_| Error::InvalidInput { field: "lease_ttl" })?;
    let result = sqlx::query(
        "UPDATE sessions AS s SET lease_expires_at = CAST(unixepoch('subsec') * 1000 AS INTEGER) + ?4 \
         WHERE s.id = ?1 AND s.lease_owner = ?2 AND s.lease_token = ?3 \
         AND s.lease_expires_at > CAST(unixepoch('subsec') * 1000 AS INTEGER) AND EXISTS (SELECT 1 FROM operations o \
         JOIN lanes l ON l.id = o.lane_id WHERE o.id = ?5 AND l.session_id = s.id \
         AND o.state = 'running' AND o.lease_token = ?3)",
    )
    .bind(lease.session_id.0)
    .bind(&lease.worker)
    .bind(lease.token)
    .bind(millis)
    .bind(lease.operation_id.0)
    .execute(pool)
    .await
    .map_err(crate::sqlite_error::store)?;
    if result.rows_affected() == 1 {
        return Ok(());
    }
    Err(Error::LeaseLost)
}
