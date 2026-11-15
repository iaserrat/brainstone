use sqlx::SqlitePool;

use crate::sqlite_schedule_parse::{Row, schedule_columns};
use crate::{Error, OperationId, ScheduleId, Scheduled};

/// Schedule `s`, created by operation `origin`, is visible to operation
/// `caller` in the lane that created it, to its owner from a private
/// conversation, and to anyone in the conversation it posts into.
macro_rules! visible {
    () => {
        "(s.lane_id = caller.lane_id \
         OR (COALESCE(caller.execution_identity->>'private', 0) <> 0 \
         AND origin.execution_identity->>'provider' = caller.execution_identity->>'provider' \
         AND origin.execution_identity->>'tenant' = caller.execution_identity->>'tenant' \
         AND origin.execution_identity->>'actor' = caller.execution_identity->>'actor') \
         OR (s.destination->'identity'->>'provider' = caller.execution_identity->>'provider' \
         AND s.destination->'identity'->>'tenant' = caller.execution_identity->>'tenant' \
         AND s.destination->'identity'->>'conversation' \
         = caller.execution_identity->>'conversation'))"
    };
}

pub(crate) async fn list(pool: &SqlitePool, caller: OperationId) -> Result<Vec<Scheduled>, Error> {
    let rows: Vec<Row> = sqlx::query_as(concat!(
        "SELECT ",
        schedule_columns!(),
        " FROM scheduled_work s JOIN operations origin ON origin.id = s.origin_operation_id \
         JOIN operations caller ON caller.id = ?1 WHERE s.state = 'pending' AND ",
        visible!(),
        " ORDER BY s.run_at_unix_ms, s.id LIMIT 1000"
    ))
    .bind(caller.0)
    .fetch_all(pool)
    .await
    .map_err(crate::sqlite_error::store)?;
    rows.iter()
        .map(crate::sqlite_schedule_parse::scheduled)
        .collect()
}

pub(crate) async fn cancel(
    pool: &SqlitePool,
    caller: OperationId,
    id: ScheduleId,
) -> Result<Scheduled, Error> {
    sqlx::query(concat!(
        "UPDATE scheduled_work AS s SET state='cancelled' FROM operations caller, operations origin ",
        "WHERE s.id=?1 AND caller.id=?2 AND origin.id=s.origin_operation_id AND s.state='pending' AND ",
        visible!()
    )).bind(id.0).bind(caller.0).execute(pool).await.map_err(crate::sqlite_error::store)?;
    let row = cancelled(pool, caller, id).await?;
    crate::sqlite_schedule_parse::scheduled(&row)
}

async fn cancelled(pool: &SqlitePool, caller: OperationId, id: ScheduleId) -> Result<Row, Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        schedule_columns!(),
        " FROM scheduled_work s JOIN operations origin ON origin.id = s.origin_operation_id \
         JOIN operations caller ON caller.id = ?2 WHERE s.id = ?1 AND s.state = 'cancelled' AND ",
        visible!()
    ))
    .bind(id.0)
    .bind(caller.0)
    .fetch_optional(pool)
    .await
    .map_err(crate::sqlite_error::store)?
    .ok_or(Error::ScheduleNotFound)
}
