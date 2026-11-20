use crate::postgres_backend::{postgres_claim, postgres_history, postgres_identity};
use std::time::Duration;

use sqlx::{PgConnection, PgPool};

use crate::{Claimed, Error, LaneId, OperationId, SessionId};

pub(crate) async fn claim(
    pool: &PgPool,
    id: OperationId,
    worker: &str,
    ttl: Duration,
) -> Result<Option<Claimed>, Error> {
    let mut transaction = pool.begin().await.map_err(Error::store)?;
    let Some((operation_id, session_id, lane_id)) = candidate(&mut transaction, id).await? else {
        transaction.commit().await.map_err(Error::store)?;
        return Ok(None);
    };
    let token = postgres_claim::acquire_lease(&mut transaction, session_id, worker, ttl).await?;
    postgres_claim::mark_running(&mut transaction, operation_id, token).await?;
    let thread = postgres_history::load(&mut transaction, lane_id, operation_id).await?;
    let tool_grant = postgres_claim::tool_grant(&mut transaction, session_id).await?;
    let identity = postgres_identity::load(&mut transaction, operation_id).await?;
    transaction.commit().await.map_err(Error::store)?;
    Ok(Some(Claimed {
        identity,
        operation: postgres_claim::running(operation_id, session_id, lane_id),
        thread,
        tool_grant,
        lease: postgres_claim::lease(operation_id, session_id, worker, token),
    }))
}

async fn candidate(
    connection: &mut PgConnection,
    id: OperationId,
) -> Result<Option<(OperationId, SessionId, LaneId)>, Error> {
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT o.id, l.session_id, o.lane_id FROM operations o \
         JOIN lanes l ON l.id = o.lane_id JOIN sessions s ON s.id = l.session_id \
         WHERE o.id = $1 AND o.state IN ('accepted', 'running') \
         AND NOT EXISTS (SELECT 1 FROM operations prior WHERE prior.lane_id = o.lane_id \
         AND prior.id < o.id AND prior.state IN ('accepted', 'running')) \
         AND (s.lease_expires_at IS NULL OR s.lease_expires_at <= clock_timestamp()) \
         FOR UPDATE OF o, l, s SKIP LOCKED",
    )
    .bind(id.0)
    .fetch_optional(connection)
    .await
    .map_err(Error::store)?;
    Ok(row.map(|(operation, session, lane)| {
        (OperationId(operation), SessionId(session), LaneId(lane))
    }))
}
