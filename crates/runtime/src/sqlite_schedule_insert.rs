use sqlx::SqliteConnection;
use sqlx::types::Json;

use crate::sqlite_schedule_parse::{MILLIS_PER_SECOND, Row, schedule_columns};
use crate::{Error, ScheduleRequest, ScheduleTime};

/// `?8` is the first run of a standing schedule, computed by the runtime.
const INSERT: &str = "INSERT INTO scheduled_work AS s (origin_operation_id, lane_id, schedule_key, prompt, \
      run_at_unix_ms, schedule_kind, schedule_value, state, delivery_target, recurrence, \
      destination, created_at) VALUES \
      (?1, ?2, ?3, ?4, CASE WHEN ?5 = 'after' THEN \
      ?11 + ?6 \
      WHEN ?5 = 'recur' THEN ?8 ELSE ?6 END, \
      ?5, ?6, 'pending', ?7, ?9, ?10, ?11) ON CONFLICT (origin_operation_id, schedule_key) \
      DO NOTHING RETURNING id";

/// `None` when another writer already holds this operation's key.
pub(crate) async fn insert(
    connection: &mut SqliteConnection,
    request: &ScheduleRequest,
) -> Result<Option<Row>, Error> {
    let origin = crate::sqlite_schedule_origin::admitted(connection, request).await?;
    let (kind, value) = spec(&request.when)?;
    let anchor = now(connection).await?;
    let (first_run_at, recurrence) = match &request.when {
        ScheduleTime::Repeat(rule) => (Some(rule.next_run_at(anchor, anchor)?), Some(crate::stored_recurrence::encode(rule)?)),
        _ => (None, None),
    };
    let id: Option<i64> = sqlx::query_scalar(INSERT)
        .bind(request.operation_id.0).bind(origin.lane).bind(&request.key).bind(&request.prompt)
        .bind(kind).bind(value).bind(origin.target).bind(first_run_at).bind(recurrence)
        .bind(request.destination.as_ref().map(Json)).bind(anchor)
        .fetch_optional(&mut *connection).await.map_err(crate::sqlite_error::store)?;
    let Some(id) = id else { return Ok(None); };
    sqlx::query_as(concat!("SELECT ", schedule_columns!(), " FROM scheduled_work s WHERE s.id=?1"))
        .bind(id).fetch_optional(connection).await.map_err(crate::sqlite_error::store)
}

async fn now(connection: &mut SqliteConnection) -> Result<i64, Error> {
    sqlx::query_scalar("SELECT CAST(unixepoch('subsec') * 1000 AS INTEGER)")
        .fetch_one(connection).await.map_err(crate::sqlite_error::store)
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
