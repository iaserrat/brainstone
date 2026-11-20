use crate::sqlite_backend::sqlite_error::store;
use crate::sqlite_backend::{
    sqlite_accept, sqlite_admit_existing, sqlite_admit_queries, sqlite_admit_result,
    sqlite_delivery, sqlite_operation, sqlite_operation_existing, sqlite_profile, sqlite_sessions,
};
use sqlx::{SqliteConnection, SqlitePool};

use crate::{Admitted, Error, LaneId, Prompt, SessionId};

pub(crate) async fn admit(
    pool: &SqlitePool,
    prompt: &Prompt,
    definitions: &[serde_json::Value],
) -> Result<Admitted, Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(store)?;
    let connection = &mut *transaction;
    let session = sqlite_sessions::id(connection, prompt).await?;
    let lane = sqlite_accept::admission_lane_id(connection, session, prompt, definitions).await?;
    let admitted = decide(connection, session, lane, prompt).await?;
    transaction.commit().await.map_err(store)?;
    Ok(admitted)
}

async fn decide(
    connection: &mut SqliteConnection,
    session: SessionId,
    lane: LaneId,
    prompt: &Prompt,
) -> Result<Admitted, Error> {
    if sqlite_admit_queries::operation_exists(connection, lane, prompt).await? {
        let accepted =
            sqlite_operation_existing::existing(connection, session, lane, prompt).await?;
        return Ok(accepted.into());
    }
    if let Some(existing) = sqlite_admit_existing::existing(connection, lane, prompt).await? {
        return Ok(existing);
    }
    if let Some(operation) = sqlite_admit_queries::active(connection, lane, prompt).await? {
        sqlite_admit_queries::insert(connection, lane, operation, prompt).await?;
        sqlite_delivery::invalidate_presentation(connection, operation).await?;
        return sqlite_admit_result::steering(connection, operation, false).await;
    }
    sqlite_profile::configure(connection, lane, prompt).await?;
    let accepted = sqlite_operation::operation(connection, session, lane, prompt).await?;
    sqlite_delivery::register(connection, accepted.operation.id, prompt).await?;
    Ok(accepted.into())
}
