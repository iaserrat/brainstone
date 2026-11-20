use crate::postgres_backend::postgres_schedule_parse;
use sqlx::PgPool;

use crate::postgres_backend::postgres_schedule_parse::{Row, schedule_columns};
use crate::{Error, OperationId, ScheduleId, Scheduled};

/// Schedule `s`, created by operation `origin`, is visible to operation
/// `caller` in the lane that created it, to its owner from a private
/// conversation, and to anyone in the conversation it posts into.
macro_rules! visible {
    () => {
        "(s.lane_id = caller.lane_id \
         OR ((caller.execution_identity->>'private') = 'true' \
         AND origin.execution_identity->>'provider' = caller.execution_identity->>'provider' \
         AND origin.execution_identity->>'tenant' = caller.execution_identity->>'tenant' \
         AND origin.execution_identity->>'actor' = caller.execution_identity->>'actor') \
         OR (s.destination->'identity'->>'provider' = caller.execution_identity->>'provider' \
         AND s.destination->'identity'->>'tenant' = caller.execution_identity->>'tenant' \
         AND s.destination->'identity'->>'conversation' \
         = caller.execution_identity->>'conversation'))"
    };
}

pub(crate) async fn list(pool: &PgPool, caller: OperationId) -> Result<Vec<Scheduled>, Error> {
    let rows: Vec<Row> = sqlx::query_as(concat!(
        "SELECT ",
        schedule_columns!(),
        " FROM scheduled_work s JOIN operations origin ON origin.id = s.origin_operation_id \
         JOIN operations caller ON caller.id = $1 WHERE s.state = 'pending' AND ",
        visible!(),
        " ORDER BY s.run_at_unix_ms, s.id LIMIT 1000"
    ))
    .bind(caller.0)
    .fetch_all(pool)
    .await
    .map_err(Error::store)?;
    rows.iter()
        .map(postgres_schedule_parse::scheduled)
        .collect()
}

pub(crate) async fn cancel(
    pool: &PgPool,
    caller: OperationId,
    id: ScheduleId,
) -> Result<Scheduled, Error> {
    let row: Option<Row> = sqlx::query_as(concat!(
        "UPDATE scheduled_work s SET state = 'cancelled' FROM operations caller, operations origin \
         WHERE s.id = $1 AND caller.id = $2 AND origin.id = s.origin_operation_id \
         AND s.state = 'pending' AND ",
        visible!(),
        " RETURNING ",
        schedule_columns!()
    ))
    .bind(id.0)
    .bind(caller.0)
    .fetch_optional(pool)
    .await
    .map_err(Error::store)?;
    let row = match row {
        Some(row) => row,
        None => cancelled(pool, caller, id).await?,
    };
    postgres_schedule_parse::scheduled(&row)
}

async fn cancelled(pool: &PgPool, caller: OperationId, id: ScheduleId) -> Result<Row, Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        schedule_columns!(),
        " FROM scheduled_work s JOIN operations origin ON origin.id = s.origin_operation_id \
         JOIN operations caller ON caller.id = $2 WHERE s.id = $1 AND s.state = 'cancelled' AND ",
        visible!()
    ))
    .bind(id.0)
    .bind(caller.0)
    .fetch_optional(pool)
    .await
    .map_err(Error::store)?
    .ok_or(Error::ScheduleNotFound)
}
