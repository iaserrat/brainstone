use crate::sqlite_backend::sqlite_error::store;

use crate::{Error, OperationId, SessionId};
use sqlx::SqliteConnection;
use std::time::Duration;
pub(crate) async fn acquire_lease(
    connection: &mut SqliteConnection,
    session_id: SessionId,
    worker: &str,
    ttl: Duration,
) -> Result<i64, Error> {
    let millis =
        i64::try_from(ttl.as_millis()).map_err(|_| Error::InvalidInput { field: "lease_ttl" })?;
    sqlx::query_scalar(
        "UPDATE sessions SET lease_owner = ?2, lease_token = lease_token + 1, \
         lease_expires_at = CAST(unixepoch('subsec') * 1000 AS INTEGER) + ?3 \
         WHERE id = ?1 RETURNING lease_token",
    )
    .bind(session_id.0)
    .bind(worker)
    .bind(millis)
    .fetch_one(connection)
    .await
    .map_err(store)
}

pub(crate) async fn mark_running(
    connection: &mut SqliteConnection,
    operation_id: OperationId,
    token: i64,
) -> Result<(), Error> {
    sqlx::query("UPDATE operations SET state = 'running', lease_token = ?2, image_claim_token = ?2 WHERE id = ?1")
    .bind(operation_id.0)
    .bind(token)
    .execute(connection)
    .await
    .map_err(store)?;
    Ok(())
}
