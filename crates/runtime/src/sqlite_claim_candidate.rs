use sqlx::SqliteConnection;

use crate::{Error, LaneId, OperationId, SessionId};

pub(crate) async fn next(
    connection: &mut SqliteConnection,
) -> Result<Option<(OperationId, SessionId, LaneId)>, Error> {
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT o.id, l.session_id, o.lane_id FROM operations o \
         JOIN lanes l ON l.id = o.lane_id JOIN sessions s ON s.id = l.session_id \
         WHERE o.state IN ('accepted', 'running') \
         AND (s.lease_expires_at IS NULL OR s.lease_expires_at <= CAST(unixepoch('subsec') * 1000 AS INTEGER)) \
         ORDER BY CASE WHEN o.state = 'running' THEN 0 ELSE 1 END, o.id \
         LIMIT 1",
    )
    .fetch_optional(connection)
    .await
    .map_err(crate::sqlite_error::store)?;
    Ok(row.map(|(operation, session, lane)| {
        (OperationId(operation), SessionId(session), LaneId(lane))
    }))
}
