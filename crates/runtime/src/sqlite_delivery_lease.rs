use std::time::Duration;

use sqlx::SqlitePool;

use crate::{DeliveryLease, Error};

pub(crate) async fn renew(
    pool: &SqlitePool,
    lease: &DeliveryLease,
    ttl: Duration,
) -> Result<(), Error> {
    let ttl =
        i64::try_from(ttl.as_millis()).map_err(|_| Error::InvalidInput { field: "lease_ttl" })?;
    let updated = sqlx::query(
        "UPDATE operation_deliveries SET lease_expires_at = CAST(unixepoch('subsec') * 1000 AS INTEGER) + \
         ?4 WHERE operation_id = ?1 AND lease_owner = ?2 \
         AND lease_token = ?3 AND lease_expires_at > CAST(unixepoch('subsec') * 1000 AS INTEGER)",
    )
    .bind(lease.operation_id.0)
    .bind(&lease.worker)
    .bind(lease.token)
    .bind(ttl)
    .execute(pool)
    .await
    .map_err(crate::sqlite_error::store)?;
    if updated.rows_affected() == 0 {
        return Err(Error::LeaseLost);
    }
    Ok(())
}
