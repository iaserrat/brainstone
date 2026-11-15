use agent::Outcome;
use sqlx::types::Json;
use sqlx::{SqliteConnection, SqlitePool};

use crate::{Error, Subagent, SubagentId};

pub(crate) async fn stop(
    pool: &SqlitePool,
    caller: crate::OperationId,
    id: SubagentId,
) -> Result<Subagent, Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(crate::sqlite_error::store)?;
    crate::sqlite_subagent_read::authorize_on(&mut transaction, caller, id).await?;
    let lane_id = lane(&mut transaction, id).await?;
    let outcome = Outcome::Failed {
        code: "agent_cancelled".to_owned(),
        message: "subagent was stopped".to_owned(),
    };
    sqlx::query(
        "UPDATE operations SET state = 'failed', outcome = ?2 \
         WHERE lane_id = ?1 AND state IN ('accepted', 'running')",
    )
    .bind(lane_id)
    .bind(Json(outcome))
    .execute(&mut *transaction)
    .await
    .map_err(crate::sqlite_error::store)?;
    fence(&mut transaction, lane_id).await?;
    let agent = crate::sqlite_subagent_read::get_on(&mut transaction, id)
        .await?
        .ok_or(Error::InvalidInput { field: "agent_id" })?;
    transaction.commit().await.map_err(crate::sqlite_error::store)?;
    Ok(agent)
}

async fn lane(connection: &mut SqliteConnection, id: SubagentId) -> Result<i64, Error> {
    sqlx::query_scalar(
        "SELECT o.lane_id FROM subagent_runs s JOIN operations o ON o.id = s.child_operation_id \
         WHERE s.id = ?1",
    )
    .bind(id.0)
    .fetch_optional(connection)
    .await
    .map_err(crate::sqlite_error::store)?
    .ok_or(Error::InvalidInput { field: "agent_id" })
}

async fn fence(connection: &mut SqliteConnection, lane_id: i64) -> Result<(), Error> {
    sqlx::query(
        "UPDATE sessions SET lease_owner = NULL, lease_token = lease_token + 1, lease_expires_at = NULL \
         WHERE id = (SELECT session_id FROM lanes WHERE id = ?1)",
    )
    .bind(lane_id)
    .execute(connection)
    .await
    .map_err(crate::sqlite_error::store)?;
    Ok(())
}
