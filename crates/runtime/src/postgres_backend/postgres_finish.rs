use agent::Outcome;
use sqlx::PgConnection;
use sqlx::types::Json;

use crate::schedule_failure::REPEAT_FAILURE_CANCEL_AFTER;
use crate::{Error, Lease};

pub(crate) async fn finish(
    connection: &mut PgConnection,
    lease: &Lease,
    outcome: &Outcome,
) -> Result<(), Error> {
    let state = match outcome {
        Outcome::Completed { .. } => "completed",
        Outcome::Failed { .. } => "failed",
        _ => return Err(Error::corrupt("unknown outcome")),
    };
    let payload = serde_json::to_value(outcome).map_err(Error::corrupt)?;
    sqlx::query("UPDATE operations SET state = $2, outcome = $3 WHERE id = $1")
        .bind(lease.operation_id.0)
        .bind(state)
        .bind(Json(payload))
        .execute(&mut *connection)
        .await
        .map_err(Error::store)?;
    settle_standing(connection, lease, state == "failed").await?;
    release(connection, lease).await
}

/// A run settles its standing schedule's failure streak in the same
/// transaction as its outcome: a success resets it, and a streak reaching the
/// cancel threshold ends the schedule.
async fn settle_standing(
    connection: &mut PgConnection,
    lease: &Lease,
    failed: bool,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE scheduled_work SET \
         consecutive_failures = CASE WHEN $2 THEN consecutive_failures + 1 ELSE 0 END, \
         state = CASE WHEN $2 AND consecutive_failures + 1 >= $3 THEN 'cancelled' ELSE state END \
         WHERE released_operation_id = $1 AND schedule_kind IN ('repeat', 'recur') AND state = 'pending'",
    )
    .bind(lease.operation_id.0)
    .bind(failed)
    .bind(REPEAT_FAILURE_CANCEL_AFTER)
    .execute(&mut *connection)
    .await
    .map_err(Error::store)?;
    Ok(())
}

async fn release(connection: &mut PgConnection, lease: &Lease) -> Result<(), Error> {
    sqlx::query(
        "UPDATE sessions SET lease_owner = NULL, lease_expires_at = NULL \
         WHERE id = $1 AND lease_owner = $2 AND lease_token = $3",
    )
    .bind(lease.session_id.0)
    .bind(&lease.worker)
    .bind(lease.token)
    .execute(connection)
    .await
    .map_err(Error::store)?;
    Ok(())
}
