use crate::postgres_backend::postgres_schedule_origin;
use sqlx::PgConnection;
use sqlx::types::Json;

use crate::postgres_backend::postgres_schedule_parse::{MILLIS_PER_SECOND, Row, schedule_columns};
use crate::{Error, Recurrence, ScheduleRequest, ScheduleTime};

/// `$8` is the first run of a standing schedule, computed by the runtime.
const INSERT: &str = concat!(
    "INSERT INTO scheduled_work AS s (origin_operation_id, lane_id, schedule_key, prompt, \
      run_at_unix_ms, schedule_kind, schedule_value, state, delivery_target, recurrence, \
      destination) VALUES \
      ($1, $2, $3, $4, CASE WHEN $5 = 'after' THEN \
      (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT + $6 \
      WHEN $5 = 'recur' THEN $8 ELSE $6 END, \
      $5, $6, 'pending', $7, $9, $10) ON CONFLICT (origin_operation_id, schedule_key) \
      DO NOTHING RETURNING ",
    schedule_columns!()
);

/// `None` when another writer already holds this operation's key.
pub(crate) async fn insert(
    connection: &mut PgConnection,
    request: &ScheduleRequest,
) -> Result<Option<Row>, Error> {
    let origin = postgres_schedule_origin::admitted(connection, request).await?;
    let (kind, value) = spec(&request.when)?;
    let (first_run_at, recurrence) = match &request.when {
        ScheduleTime::Repeat(recurrence) => standing(connection, recurrence).await?,
        _ => (None, None),
    };
    sqlx::query_as(INSERT)
        .bind(request.operation_id.0)
        .bind(origin.lane)
        .bind(&request.key)
        .bind(&request.prompt)
        .bind(kind)
        .bind(value)
        .bind(origin.target)
        .bind(first_run_at)
        .bind(recurrence)
        .bind(request.destination.as_ref().map(Json))
        .fetch_optional(connection)
        .await
        .map_err(Error::store)
}

/// The first run and stored rule of a standing schedule. Its anchor is the
/// row's `created_at`, the transaction's timestamp, read here in the same
/// transaction so the first run and every later one count from one instant.
async fn standing(
    connection: &mut PgConnection,
    recurrence: &Recurrence,
) -> Result<(Option<i64>, Option<Json<serde_json::Value>>), Error> {
    let anchor: i64 =
        sqlx::query_scalar("SELECT (EXTRACT(EPOCH FROM transaction_timestamp()) * 1000)::BIGINT")
            .fetch_one(connection)
            .await
            .map_err(Error::store)?;
    let first_run_at = recurrence.next_run_at(anchor, anchor)?;
    let stored = crate::stored_recurrence::encode(recurrence)?;
    Ok((Some(first_run_at), Some(stored)))
}

/// The row's kind and value columns for `when`.
fn spec(when: &ScheduleTime) -> Result<(&'static str, i64), Error> {
    match when {
        ScheduleTime::AtUnixMillisecond(value) => Ok(("at", *value)),
        ScheduleTime::AfterSeconds(value) => i64::try_from(*value)
            .ok()
            .and_then(|value| value.checked_mul(MILLIS_PER_SECOND))
            .map(|value| ("after", value))
            .ok_or(Error::InvalidInput {
                field: "after_seconds",
            }),
        ScheduleTime::Repeat(_) => Ok(("recur", 0)),
    }
}
