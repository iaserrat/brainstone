use std::time::Duration;

use sqlx::PgPool;

use crate::{DeliveryLease, Error};

pub(crate) async fn renew(
    pool: &PgPool,
    lease: &DeliveryLease,
    ttl: Duration,
) -> Result<(), Error> {
    let ttl =
        i64::try_from(ttl.as_millis()).map_err(|_| Error::InvalidInput { field: "lease_ttl" })?;
    let updated = sqlx::query(
        "UPDATE operation_deliveries SET lease_expires_at = clock_timestamp() + \
         ($4 * INTERVAL '1 millisecond') WHERE operation_id = $1 AND lease_owner = $2 \
         AND lease_token = $3 AND lease_expires_at > clock_timestamp()",
    )
    .bind(lease.operation_id.0)
    .bind(&lease.worker)
    .bind(lease.token)
    .bind(ttl)
    .execute(pool)
    .await
    .map_err(Error::store)?;
    if updated.rows_affected() == 0 {
        return Err(Error::LeaseLost);
    }
    Ok(())
}
