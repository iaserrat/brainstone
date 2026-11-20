use crate::postgres_backend::{
    postgres_claim_candidate, postgres_history, postgres_identity, postgres_schedule_release,
};
use std::time::Duration;

use sqlx::types::Json;
use sqlx::{PgConnection, PgPool};

#[path = "postgres_claim_lease.rs"]
mod acquire;
pub(crate) use acquire::{acquire_lease, mark_running};

use crate::{Claimed, Error, LaneId, Lease, Operation, OperationId, OperationState, SessionId};

pub(crate) async fn claim(
    pool: &PgPool,
    worker: &str,
    ttl: Duration,
) -> Result<Option<Claimed>, Error> {
    let mut transaction = pool.begin().await.map_err(Error::store)?;
    let connection = &mut *transaction;
    postgres_schedule_release::one(connection).await?;
    let Some((operation_id, session_id, lane_id)) =
        postgres_claim_candidate::next(connection).await?
    else {
        transaction.commit().await.map_err(Error::store)?;
        return Ok(None);
    };
    let token = acquire_lease(connection, session_id, worker, ttl).await?;
    mark_running(connection, operation_id, token).await?;
    let thread = postgres_history::load(connection, lane_id, operation_id).await?;
    let tool_grant = tool_grant(connection, session_id).await?;
    let identity = postgres_identity::load(&mut transaction, operation_id).await?;
    transaction.commit().await.map_err(Error::store)?;
    Ok(Some(Claimed {
        identity,
        operation: running(operation_id, session_id, lane_id),
        thread,
        tool_grant,
        lease: lease(operation_id, session_id, worker, token),
    }))
}

pub(crate) fn lease(
    operation_id: OperationId,
    session_id: SessionId,
    worker: &str,
    token: i64,
) -> Lease {
    Lease {
        operation_id,
        session_id,
        worker: worker.to_owned(),
        token,
    }
}

/// A subagent's session carries its spawn grant; any other session has none.
pub(crate) async fn tool_grant(
    connection: &mut PgConnection,
    session_id: SessionId,
) -> Result<Option<Vec<String>>, Error> {
    let tools: Option<Json<Vec<serde_json::Value>>> = sqlx::query_scalar(
        "SELECT request->'config'->'tools' FROM subagent_runs WHERE child_session_id = $1",
    )
    .bind(session_id.0)
    .fetch_optional(connection)
    .await
    .map_err(Error::store)?;
    Ok(tools.map(|Json(tools)| crate::subagent_config::tool_names(&tools)))
}

pub(crate) fn running(id: OperationId, session_id: SessionId, lane_id: LaneId) -> Operation {
    Operation {
        id,
        session_id,
        lane_id,
        state: OperationState::Running,
        outcome: None,
    }
}
