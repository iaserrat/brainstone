use sqlx::types::Json;
use sqlx::{SqliteConnection, SqlitePool};

use crate::{Accepted, Error, LaneId, Prompt, SessionId};

pub(crate) async fn accept(
    pool: &SqlitePool,
    prompt: &Prompt,
    definitions: &[serde_json::Value],
) -> Result<Accepted, Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(crate::sqlite_error::store)?;
    let accepted = accept_in(&mut transaction, prompt, definitions).await?;
    transaction.commit().await.map_err(crate::sqlite_error::store)?;
    Ok(accepted)
}

pub(crate) async fn accept_in(
    connection: &mut SqliteConnection,
    prompt: &Prompt,
    definitions: &[serde_json::Value],
) -> Result<Accepted, Error> {
    let session_id = crate::sqlite_sessions::id(connection, prompt).await?;
    let lane_id = lane_id(connection, session_id, prompt, definitions).await?;
    let accepted =
        crate::sqlite_operation::operation(connection, session_id, lane_id, prompt).await?;
    crate::sqlite_delivery::register(connection, accepted.operation.id, prompt).await?;
    Ok(accepted)
}

pub(crate) async fn lane_id(
    connection: &mut SqliteConnection,
    session_id: SessionId,
    prompt: &Prompt,
    definitions: &[serde_json::Value],
) -> Result<LaneId, Error> {
    let inserted = insert_lane(connection, session_id, prompt, definitions).await?;
    if let Some(id) = inserted {
        return Ok(LaneId(id));
    }
    crate::sqlite_lane_resolution::resolve(
        connection,
        session_id,
        prompt,
        crate::sqlite_lane_resolution::Mode::Strict,
    )
    .await
}

pub(crate) async fn admission_lane_id(
    connection: &mut SqliteConnection,
    session_id: SessionId,
    prompt: &Prompt,
    definitions: &[serde_json::Value],
) -> Result<LaneId, Error> {
    let inserted = insert_lane(connection, session_id, prompt, definitions).await?;
    match inserted {
        Some(id) => Ok(LaneId(id)),
        None => {
            crate::sqlite_lane_resolution::resolve(
                connection,
                session_id,
                prompt,
                crate::sqlite_lane_resolution::Mode::SurfaceAdmission,
            )
            .await
        }
    }
}

// remove_me_hackyhack: `tool_definitions` serves rollback only (older binaries read it).
// Delete it and `Store` admission's `definitions` once no such binary can deploy.
async fn insert_lane(
    connection: &mut SqliteConnection,
    session_id: SessionId,
    prompt: &Prompt,
    definitions: &[serde_json::Value],
) -> Result<Option<i64>, Error> {
    sqlx::query_scalar(
        "INSERT INTO lanes (session_id, external_id, system_prompt, tool_definitions, execution_profile, execution_identity) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
         ON CONFLICT (session_id, external_id) DO NOTHING RETURNING id",
    )
    .bind(session_id.0)
    .bind(&prompt.address.lane)
    .bind(&prompt.system)
    .bind(Json(definitions))
    .bind(Json(&prompt.profile))
    .bind(prompt.identity.as_ref().map(Json))
    .fetch_optional(connection)
    .await
    .map_err(crate::sqlite_error::store)
}
