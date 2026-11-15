use std::time::Duration;

use sqlx::PgPool;

use crate::{Error, Lease};

pub(crate) async fn heartbeat(pool: &PgPool, lease: &Lease, ttl: Duration) -> Result<(), Error> {
    let millis =
        i64::try_from(ttl.as_millis()).map_err(|_| Error::InvalidInput { field: "lease_ttl" })?;
    let result = sqlx::query(
        "UPDATE sessions s SET lease_expires_at = clock_timestamp() + ($4 * INTERVAL '1 millisecond') \
         WHERE s.id = $1 AND s.lease_owner = $2 AND s.lease_token = $3 \
         AND s.lease_expires_at > clock_timestamp() AND EXISTS (SELECT 1 FROM operations o \
         JOIN lanes l ON l.id = o.lane_id WHERE o.id = $5 AND l.session_id = s.id \
         AND o.state = 'running' AND o.lease_token = $3)",
    )
    .bind(lease.session_id.0)
    .bind(&lease.worker)
    .bind(lease.token)
    .bind(millis)
    .bind(lease.operation_id.0)
    .execute(pool)
    .await
    .map_err(Error::store)?;
    if result.rows_affected() == 1 {
        return Ok(());
    }
    Err(Error::LeaseLost)
}
