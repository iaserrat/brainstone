use super::HistoryRow;
use crate::postgres_backend::postgres_history_boundary::Boundary;
use crate::{Error, LaneId, OperationId};
use agent::ExecutionProfile;
use sqlx::{PgConnection, types::Json};
pub(super) async fn configuration(
    connection: &mut PgConnection,
    lane_id: LaneId,
) -> Result<(String, ExecutionProfile), Error> {
    let (system, Json(profile)) =
        sqlx::query_as("SELECT system_prompt, execution_profile FROM lanes WHERE id = $1")
            .bind(lane_id.0)
            .fetch_one(connection)
            .await
            .map_err(Error::store)?;
    Ok((system, profile))
}

pub(super) async fn compaction(
    connection: &mut PgConnection,
    lane_id: LaneId,
    through: OperationId,
) -> Result<Option<(i64, String)>, Error> {
    sqlx::query_as(
        "SELECT through_operation_id, summary FROM lane_compactions \
         WHERE lane_id = $1 AND through_operation_id < $2 ORDER BY id DESC LIMIT 1",
    )
    .bind(lane_id.0)
    .bind(through.0)
    .fetch_optional(connection)
    .await
    .map_err(Error::store)
}

pub(super) async fn rows(
    connection: &mut PgConnection,
    lane_id: LaneId,
    through: OperationId,
    boundary: Boundary,
) -> Result<Vec<HistoryRow>, Error> {
    sqlx::query_as(
        "SELECT o.id, -1::BIGINT, o.prompt, NULL::JSONB, 1::SMALLINT \
         FROM operations o WHERE o.lane_id = $1 AND o.id <= $2 AND o.id > $3 \
         UNION ALL SELECT e.operation_id, e.sequence, NULL::TEXT, e.payload, e.schema_version \
         FROM operation_events e JOIN operations o ON o.id = e.operation_id \
         WHERE o.lane_id = $1 AND o.id <= $2 AND (o.id > $3 OR \
         (o.id = $3 AND e.sequence >= $4)) ORDER BY 1, 2",
    )
    .bind(lane_id.0)
    .bind(through.0)
    .bind(boundary.operation)
    .bind(boundary.sequence)
    .fetch_all(connection)
    .await
    .map_err(Error::store)
}

pub(super) async fn tool_evidence(
    connection: &mut PgConnection,
    operation: OperationId,
    required: bool,
) -> Result<crate::tool_evidence::Evidence, Error> {
    if !required {
        return legacy_evidence(connection, operation).await;
    }
    let rows = sqlx::query_as::<_, crate::tool_evidence::EvidenceRow>(
        "SELECT schema_version,payload FROM operation_events WHERE operation_id=$1 ORDER BY sequence",
    )
    .bind(operation.get())
    .fetch_all(connection)
    .await
    .map_err(Error::store)?;
    crate::tool_evidence::Evidence::decode(rows)
}

// Without success requirements, compaction still invalidates old event bodies.
// Preserve the historical identity counter without interpreting those bodies.
async fn legacy_evidence(
    connection: &mut PgConnection,
    operation: OperationId,
) -> Result<crate::tool_evidence::Evidence, Error> {
    let count = sqlx::query_scalar(
        "SELECT count(*) FROM operation_events WHERE operation_id=$1 \
         AND payload->>'event'='appended' AND payload->'message'->'Assistant' IS NOT NULL",
    )
    .bind(operation.get())
    .fetch_one(connection)
    .await
    .map_err(Error::store)?;
    crate::tool_evidence::Evidence::responses(count)
}
