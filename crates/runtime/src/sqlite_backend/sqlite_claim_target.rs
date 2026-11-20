use crate::sqlite_backend::sqlite_error::store;
use crate::sqlite_backend::{sqlite_claim, sqlite_history, sqlite_identity};
use std::time::Duration;

use sqlx::{SqliteConnection, SqlitePool};

use crate::{Claimed, Error, LaneId, OperationId, SessionId};

pub(crate) async fn claim(
    pool: &SqlitePool,
    id: OperationId,
    worker: &str,
    ttl: Duration,
) -> Result<Option<Claimed>, Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(store)?;
    let Some((operation_id, session_id, lane_id)) = candidate(&mut transaction, id).await? else {
        transaction.commit().await.map_err(store)?;
        return Ok(None);
    };
    let token = sqlite_claim::acquire_lease(&mut transaction, session_id, worker, ttl).await?;
    sqlite_claim::mark_running(&mut transaction, operation_id, token).await?;
    let thread = sqlite_history::load(&mut transaction, lane_id, operation_id).await?;
    let tool_grant = sqlite_claim::tool_grant(&mut transaction, session_id).await?;
    let identity = sqlite_identity::load(&mut transaction, operation_id).await?;
    transaction.commit().await.map_err(store)?;
    Ok(Some(Claimed {
        identity,
        operation: sqlite_claim::running(operation_id, session_id, lane_id),
        thread,
        tool_grant,
        lease: sqlite_claim::lease(operation_id, session_id, worker, token),
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
    .map_err(store)?;
    Ok(row.map(|(operation, session, lane)| {
        (OperationId(operation), SessionId(session), LaneId(lane))
    }))
}
