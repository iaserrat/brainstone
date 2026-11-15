use sqlx::types::Json;
use sqlx::{PgConnection, PgPool};

use crate::{Error, SpawnedSubagent, SubagentId, SubagentRequest};

pub(crate) async fn spawn(
    pool: &PgPool,
    request: &SubagentRequest,
) -> Result<SpawnedSubagent, Error> {
    let mut transaction = pool.begin().await.map_err(Error::store)?;
    if let Some(spawned) = crate::postgres_subagent_existing::get(&mut transaction, request).await?
    {
        transaction.commit().await.map_err(Error::store)?;
        return Ok(spawned);
    }
    let root = crate::postgres_subagent_limit::lock(&mut transaction, request.parent).await?;
    if let Some(spawned) = crate::postgres_subagent_existing::get(&mut transaction, request).await?
    {
        transaction.commit().await.map_err(Error::store)?;
        return Ok(spawned);
    }
    crate::postgres_subagent_limit::check(&mut transaction, root).await?;
    let id = insert(&mut transaction, root, request).await?;
    let spawned = create(&mut transaction, SubagentId(id), request).await?;
    transaction.commit().await.map_err(Error::store)?;
    Ok(spawned)
}

async fn insert(
    connection: &mut PgConnection,
    root_session_id: i64,
    request: &SubagentRequest,
) -> Result<i64, Error> {
    sqlx::query_scalar(
        "INSERT INTO subagent_runs (parent_operation_id, root_session_id, spawn_key, request) \
         VALUES ($1, $2, $3, $4) \
         RETURNING id",
    )
    .bind(request.parent.0)
    .bind(root_session_id)
    .bind(&request.key)
    .bind(Json(request))
    .fetch_one(connection)
    .await
    .map_err(Error::store)
}

async fn create(
    connection: &mut PgConnection,
    id: SubagentId,
    request: &SubagentRequest,
) -> Result<SpawnedSubagent, Error> {
    let prompt = crate::postgres_subagent_existing::prompt(id, request)?;
    let prompt = crate::postgres_identity::inherit(connection, request.parent, prompt).await?;
    let accepted =
        crate::postgres_accept::accept_in(connection, &prompt, &request.config.tools).await?;
    sqlx::query(
        "UPDATE subagent_runs SET child_operation_id = $2, child_session_id = $3 WHERE id = $1",
    )
    .bind(id.0)
    .bind(accepted.operation.id.0)
    .bind(accepted.operation.session_id.0)
    .execute(connection)
    .await
    .map_err(Error::store)?;
    Ok(crate::postgres_subagent_existing::spawned(
        id,
        request,
        accepted.operation,
        false,
    ))
}
