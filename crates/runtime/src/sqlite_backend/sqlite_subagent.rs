use crate::sqlite_backend::sqlite_error::store;
use crate::sqlite_backend::{
    sqlite_accept, sqlite_identity, sqlite_subagent_existing, sqlite_subagent_limit,
};
use sqlx::types::Json;
use sqlx::{SqliteConnection, SqlitePool};

use crate::{Error, SpawnedSubagent, SubagentId, SubagentRequest};

pub(crate) async fn spawn(
    pool: &SqlitePool,
    request: &SubagentRequest,
) -> Result<SpawnedSubagent, Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(store)?;
    if let Some(spawned) = sqlite_subagent_existing::get(&mut transaction, request).await? {
        transaction.commit().await.map_err(store)?;
        return Ok(spawned);
    }
    let root = sqlite_subagent_limit::root(&mut transaction, request.parent).await?;
    sqlite_subagent_limit::check(&mut transaction, root).await?;
    let id = insert(&mut transaction, root, request).await?;
    let spawned = create(&mut transaction, SubagentId(id), request).await?;
    transaction.commit().await.map_err(store)?;
    Ok(spawned)
}

async fn insert(
    connection: &mut SqliteConnection,
    root_session_id: i64,
    request: &SubagentRequest,
) -> Result<i64, Error> {
    sqlx::query_scalar(
        "INSERT INTO subagent_runs (parent_operation_id, root_session_id, spawn_key, request) \
         VALUES (?1, ?2, ?3, ?4) \
         RETURNING id",
    )
    .bind(request.parent.0)
    .bind(root_session_id)
    .bind(&request.key)
    .bind(Json(request))
    .fetch_one(connection)
    .await
    .map_err(store)
}

async fn create(
    connection: &mut SqliteConnection,
    id: SubagentId,
    request: &SubagentRequest,
) -> Result<SpawnedSubagent, Error> {
    let prompt = sqlite_subagent_existing::prompt(id, request)?;
    let prompt = sqlite_identity::inherit(connection, request.parent, prompt).await?;
    let accepted = sqlite_accept::accept_in(connection, &prompt, &request.config.tools).await?;
    sqlx::query(
        "UPDATE subagent_runs SET child_operation_id = ?2, child_session_id = ?3 WHERE id = ?1",
    )
    .bind(id.0)
    .bind(accepted.operation.id.0)
    .bind(accepted.operation.session_id.0)
    .execute(connection)
    .await
    .map_err(store)?;
    Ok(sqlite_subagent_existing::spawned(
        id,
        request,
        accepted.operation,
        false,
    ))
}
