use std::time::Duration;

use sqlx::{SqliteConnection, SqlitePool};

use crate::{Claimed, Error, LaneId, OperationId, SessionId};

pub(crate) async fn claim(
    pool: &SqlitePool,
    id: OperationId,
    worker: &str,
    ttl: Duration,
) -> Result<Option<Claimed>, Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(crate::sqlite_error::store)?;
    let Some((operation_id, session_id, lane_id)) = candidate(&mut transaction, id).await? else {
        transaction.commit().await.map_err(crate::sqlite_error::store)?;
        return Ok(None);
    };
    let token =
        crate::sqlite_claim::acquire_lease(&mut transaction, session_id, worker, ttl).await?;
    crate::sqlite_claim::mark_running(&mut transaction, operation_id, token).await?;
    let thread = crate::sqlite_history::load(&mut transaction, lane_id, operation_id).await?;
    let tool_grant = crate::sqlite_claim::tool_grant(&mut transaction, session_id).await?;
    let identity = crate::sqlite_identity::load(&mut transaction, operation_id).await?;
    transaction.commit().await.map_err(crate::sqlite_error::store)?;
    Ok(Some(Claimed {
        identity,
        operation: crate::sqlite_claim::running(operation_id, session_id, lane_id),
        thread,
        tool_grant,
        lease: crate::sqlite_claim::lease(operation_id, session_id, worker, token),
    }))
}

async fn candidate(
    connection: &mut SqliteConnection,
    id: OperationId,
) -> Result<Option<(OperationId, SessionId, LaneId)>, Error> {
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT o.id, l.session_id, o.lane_id FROM operations o \
         JOIN lanes l ON l.id = o.lane_id JOIN sessions s ON s.id = l.session_id \
         WHERE o.id = ?1 AND o.state IN ('accepted', 'running') \
         AND NOT EXISTS (SELECT 1 FROM operations prior WHERE prior.lane_id = o.lane_id \
         AND prior.id < o.id AND prior.state IN ('accepted', 'running')) \
         AND (s.lease_expires_at IS NULL OR s.lease_expires_at <= CAST(unixepoch('subsec') * 1000 AS INTEGER)) \
        ",
    )
    .bind(id.0)
    .fetch_optional(connection)
    .await
    .map_err(crate::sqlite_error::store)?;
    Ok(row.map(|(operation, session, lane)| {
        (OperationId(operation), SessionId(session), LaneId(lane))
    }))
}
