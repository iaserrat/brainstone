use crate::postgres_backend::postgres_read;
use sqlx::types::Json;
use sqlx::{PgConnection, PgPool};

use crate::{Error, Subagent, SubagentId, SubagentRequest};

type Row = (Json<SubagentRequest>, Option<i64>);

pub(crate) async fn get(
    pool: &PgPool,
    caller: crate::OperationId,
    id: SubagentId,
) -> Result<Option<Subagent>, Error> {
    let mut connection = pool.acquire().await.map_err(Error::store)?;
    authorize_on(&mut connection, caller, id).await?;
    get_on(&mut connection, id).await
}

pub(crate) async fn authorize_on(
    connection: &mut PgConnection,
    caller: crate::OperationId,
    id: SubagentId,
) -> Result<(), Error> {
    let allowed: Option<bool> = sqlx::query_scalar(
        "SELECT target.root_session_id = COALESCE(caller_agent.root_session_id, caller_lane.session_id) \
         AND caller.execution_identity IS NOT DISTINCT FROM parent.execution_identity \
         FROM subagent_runs target JOIN operations parent ON parent.id = target.parent_operation_id \
         JOIN operations caller ON caller.id = $2 \
         JOIN lanes caller_lane ON caller_lane.id = caller.lane_id \
         LEFT JOIN subagent_runs caller_agent ON caller_agent.child_session_id = caller_lane.session_id \
         WHERE target.id = $1",
    )
    .bind(id.0)
    .bind(caller.0)
    .fetch_optional(connection)
    .await
    .map_err(Error::store)?;
    if allowed == Some(true) {
        return Ok(());
    }
    Err(Error::InvalidInput { field: "agent_id" })
}

pub(crate) async fn get_on(
    connection: &mut PgConnection,
    id: SubagentId,
) -> Result<Option<Subagent>, Error> {
    let row: Option<Row> =
        sqlx::query_as("SELECT request, child_operation_id FROM subagent_runs WHERE id = $1")
            .bind(id.0)
            .fetch_optional(&mut *connection)
            .await
            .map_err(Error::store)?;
    let Some((Json(request), operation)) = row else {
        return Ok(None);
    };
    let operation = operation.ok_or_else(|| Error::CorruptRecord {
        message: "subagent has no child operation".to_owned(),
    })?;
    let operation = postgres_read::operation_on(connection, crate::OperationId(operation))
        .await?
        .ok_or_else(|| Error::CorruptRecord {
            message: "subagent operation is missing".to_owned(),
        })?;
    Ok(Some(Subagent {
        id,
        operation,
        name: request.config.name,
        background: request.config.background,
    }))
}
