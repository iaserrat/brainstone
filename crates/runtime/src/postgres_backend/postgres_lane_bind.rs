use sqlx::types::Json;
use sqlx::{PgConnection, PgPool};

use crate::{DeliveryLease, DeliveryTarget, Error};

/// `(lane_id, request_id, schedule_id, lane external_id)` of the fenced operation.
type Fenced = (i64, String, Option<i64>, String);

/// The lane of a post's run takes the name of the root its surface posted,
/// and the operation's remaining delivery moves to the reply route.
pub(crate) async fn bind(
    pool: &PgPool,
    lease: &DeliveryLease,
    external_id: &str,
    target: &DeliveryTarget,
) -> Result<bool, Error> {
    let mut transaction = pool.begin().await.map_err(Error::store)?;
    let (lane, provisional, schedule, current) = fence(&mut transaction, lease).await?;
    if current == external_id {
        return Ok(true);
    }
    if schedule.is_none() || current != provisional {
        return Err(Error::InvalidInput { field: "lane" });
    }
    if !rename(&mut transaction, lane, external_id).await? {
        return Ok(false);
    }
    sqlx::query("UPDATE operations SET delivery_target = $2 WHERE id = $1")
        .bind(lease.operation_id.0)
        .bind(Json(target))
        .execute(&mut *transaction)
        .await
        .map_err(Error::store)?;
    transaction.commit().await.map_err(Error::store)?;
    Ok(true)
}

/// `false` when another lane of the session already holds `external_id`.
async fn rename(
    connection: &mut PgConnection,
    lane: i64,
    external_id: &str,
) -> Result<bool, Error> {
    let renamed = sqlx::query("UPDATE lanes SET external_id = $2 WHERE id = $1")
        .bind(lane)
        .bind(external_id)
        .execute(connection)
        .await;
    match renamed {
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => Ok(false),
        other => other.map(|_| true).map_err(Error::store),
    }
}

async fn fence(connection: &mut PgConnection, lease: &DeliveryLease) -> Result<Fenced, Error> {
    sqlx::query_as(
        "SELECT o.lane_id, o.request_id, o.schedule_id, l.external_id \
         FROM operation_deliveries d JOIN operations o ON o.id = d.operation_id \
         JOIN lanes l ON l.id = o.lane_id WHERE d.operation_id = $1 AND d.lease_owner = $2 \
         AND d.lease_token = $3 AND d.lease_expires_at > clock_timestamp() FOR UPDATE OF d, l",
    )
    .bind(lease.operation_id.0)
    .bind(&lease.worker)
    .bind(lease.token)
    .fetch_optional(connection)
    .await
    .map_err(Error::store)?
    .ok_or(Error::LeaseLost)
}
