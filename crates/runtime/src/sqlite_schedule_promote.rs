//! Promote one due `scheduled_work` row into an accepted operation in its lane.

use sqlx::SqliteConnection;
use sqlx::types::Json;

use crate::Error;

/// `(id, lane_id, prompt, delivery_target, run_at_unix_ms, repeat_minute,
/// repeat_time_zone, repeat_days, recurrence, created_at_unix_ms,
/// destination)` for the row being released.
pub(crate) type Due = (
    i64,
    i64,
    String,
    Option<Json<serde_json::Value>>,
    i64,
    Option<i16>,
    Option<String>,
    Option<Json<Vec<i16>>>,
    Option<Json<serde_json::Value>>,
    i64,
    Option<Json<serde_json::Value>>,
);

/// A standing schedule's runs share a lane, so each carries its run time.
fn request_id(row: &Due, repeats: bool) -> String {
    if repeats {
        return format!("schedule/{}/{}", row.0, row.4);
    }
    format!("schedule/{}", row.0)
}

/// The released operation's id, and whether it has a delivery target. A run
/// replies in the origin lane as its origin; a post's run opens a lane of its
/// own and runs as the destination's principal. `?6` is that principal, or
/// NULL for the origin's.
pub(crate) async fn insert(
    connection: &mut SqliteConnection,
    row: &Due,
    repeats: bool,
) -> Result<(i64, bool), Error> {
    let request_id = request_id(row, repeats);
    let (lane, target, identity) = route(connection, row, &request_id).await?;
    let id = sqlx::query_scalar(
        "INSERT INTO operations (lane_id, request_id, prompt, source_prompt, state, delivery_target, image_count, execution_identity, schedule_id) \
         VALUES (?1, ?2, ?3, ?3, 'accepted', ?4, 0, COALESCE(?6, (SELECT o.execution_identity FROM operations o JOIN scheduled_work w ON w.origin_operation_id = o.id WHERE w.id = ?5)), ?5) RETURNING id",
    )
    .bind(lane)
    .bind(request_id)
    .bind(&row.2)
    .bind(&target)
    .bind(row.0)
    .bind(identity)
    .fetch_one(connection)
    .await
    .map_err(crate::sqlite_error::store)?;
    Ok((id, target.is_some()))
}

/// `(lane, delivery target, principal)` of the run: the origin's own, or a
/// new lane in the destination with its target and principal.
async fn route(
    connection: &mut SqliteConnection,
    row: &Due,
    request_id: &str,
) -> Result<(i64, Option<Json<serde_json::Value>>, Option<Json<serde_json::Value>>), Error> {
    let destination = row
        .10
        .as_ref()
        .map(|Json(value)| serde_json::from_value::<crate::Destination>(value.clone()))
        .transpose()
        .map_err(Error::corrupt)?;
    let Some(destination) = destination else {
        return Ok((row.1, row.3.clone(), None));
    };
    let lane = crate::sqlite_schedule_lane::open(connection, row.1, &destination, request_id).await?;
    let target = serde_json::to_value(&destination.target).map_err(Error::corrupt)?;
    let identity = serde_json::to_value(&destination.identity).map_err(Error::corrupt)?;
    Ok((lane, Some(Json(target)), Some(Json(identity))))
}

pub(crate) async fn register_delivery(
    connection: &mut SqliteConnection,
    operation: i64,
    required: bool,
) -> Result<(), Error> {
    if !required {
        return Ok(());
    }
    sqlx::query("INSERT INTO operation_deliveries (operation_id) VALUES (?1)")
        .bind(operation)
        .execute(connection)
        .await
        .map_err(crate::sqlite_error::store)?;
    Ok(())
}
