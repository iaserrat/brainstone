use crate::sqlite_backend::sqlite_error::store;
use crate::sqlite_backend::sqlite_schedule_promote;
use sqlx::SqliteConnection;

use crate::Error;
use crate::sqlite_backend::sqlite_schedule_promote::Due;

pub(crate) async fn one(connection: &mut SqliteConnection) -> Result<(), Error> {
    let Some(row) = due(connection).await? else {
        return Ok(());
    };
    let legacy = (
        row.5,
        row.6.as_deref(),
        row.7.as_ref().map(|days| days.0.as_slice()),
    );
    let repeat = crate::stored_recurrence::decode(row.8.as_ref(), legacy)?;
    let (operation, delivers) =
        sqlite_schedule_promote::insert(connection, &row, repeat.is_some()).await?;
    sqlite_schedule_promote::register_delivery(connection, operation, delivers).await?;
    match repeat {
        Some(recurrence) => {
            let next_run_at = recurrence.next_run_at(database_now_ms(connection).await?, row.9)?;
            advance(connection, row.0, operation, next_run_at).await
        }
        None => mark_released(connection, row.0, operation).await,
    }
}

/// Due-ness is judged by database time, so the next run is computed from it
/// too: it lands strictly after the run just released, whatever a worker's
/// clock says, and `schedule/<id>/<run_at>` request ids never collide.
async fn database_now_ms(connection: &mut SqliteConnection) -> Result<i64, Error> {
    sqlx::query_scalar("SELECT CAST(unixepoch('subsec') * 1000 AS INTEGER)")
        .fetch_one(connection)
        .await
        .map_err(store)
}

/// A standing schedule whose previous run has not ended is not due: runs never
/// overlap, and its failure streak counts whole runs.
async fn due(connection: &mut SqliteConnection) -> Result<Option<Due>, Error> {
    sqlx::query_as(
        "SELECT w.id, w.lane_id, w.prompt, w.delivery_target, w.run_at_unix_ms, \
         w.repeat_minute, w.repeat_time_zone, w.repeat_days, w.recurrence, \
         w.created_at, w.destination FROM scheduled_work w \
         WHERE w.state = 'pending' AND w.run_at_unix_ms <= \
         CAST(unixepoch('subsec') * 1000 AS INTEGER) \
         AND NOT EXISTS (SELECT 1 FROM operations o WHERE o.id = w.released_operation_id \
         AND o.state IN ('accepted', 'running')) \
         AND (w.destination IS NOT NULL \
         OR (SELECT l.follow_enabled FROM lanes l WHERE l.id = w.lane_id)) \
         ORDER BY w.run_at_unix_ms, w.id LIMIT 1",
    )
    .fetch_optional(connection)
    .await
    .map_err(store)
}

async fn advance(
    connection: &mut SqliteConnection,
    id: i64,
    operation: i64,
    next_run_at: i64,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE scheduled_work SET run_at_unix_ms = ?3, released_operation_id = ?2 WHERE id = ?1",
    )
    .bind(id)
    .bind(operation)
    .bind(next_run_at)
    .execute(connection)
    .await
    .map_err(store)?;
    Ok(())
}

async fn mark_released(
    connection: &mut SqliteConnection,
    id: i64,
    operation: i64,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE scheduled_work SET state = 'released', released_operation_id = ?2 WHERE id = ?1",
    )
    .bind(id)
    .bind(operation)
    .execute(connection)
    .await
    .map_err(store)?;
    Ok(())
}
