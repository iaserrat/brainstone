use agent::ExecutionProfile;
use sqlx::{SqliteConnection, types::Json};
use crate::{Error, LaneId, OperationId};
use crate::sqlite_history_boundary::Boundary;
use super::HistoryRow;
pub(super) async fn configuration(
    connection: &mut SqliteConnection,
    lane_id: LaneId,
) -> Result<(String, ExecutionProfile), Error> {
    let (system, Json(profile)) =
        sqlx::query_as("SELECT system_prompt, execution_profile FROM lanes WHERE id = ?1")
            .bind(lane_id.0)
            .fetch_one(connection)
            .await
            .map_err(crate::sqlite_error::store)?;
    Ok((system, profile))
}

pub(super) async fn compaction(
    connection: &mut SqliteConnection,
    lane_id: LaneId,
    through: OperationId,
) -> Result<Option<(i64, String)>, Error> {
    sqlx::query_as(
        "SELECT through_operation_id, summary FROM lane_compactions \
         WHERE lane_id = ?1 AND through_operation_id < ?2 ORDER BY id DESC LIMIT 1",
    )
    .bind(lane_id.0)
    .bind(through.0)
    .fetch_optional(connection)
    .await
    .map_err(crate::sqlite_error::store)
}

pub(super) async fn rows(
    connection: &mut SqliteConnection,
    lane_id: LaneId,
    through: OperationId,
    boundary: Boundary,
) -> Result<Vec<HistoryRow>, Error> {
    sqlx::query_as(
        "SELECT o.id, -1, o.prompt, NULL, 1 \
         FROM operations o WHERE o.lane_id = ?1 AND o.id <= ?2 AND o.id > ?3 \
         UNION ALL SELECT e.operation_id, e.sequence, NULL, e.payload, e.schema_version \
         FROM operation_events e JOIN operations o ON o.id = e.operation_id \
         WHERE o.lane_id = ?1 AND o.id <= ?2 AND (o.id > ?3 OR \
         (o.id = ?3 AND e.sequence >= ?4)) ORDER BY 1, 2",
    )
    .bind(lane_id.0)
    .bind(through.0)
    .bind(boundary.operation)
    .bind(boundary.sequence)
    .fetch_all(connection)
    .await
    .map_err(crate::sqlite_error::store)
}
