use sqlx::{SqliteConnection, SqlitePool};

use crate::{Admitted, Error, LaneId, Prompt, SessionId};

pub(crate) async fn admit(
    pool: &SqlitePool,
    prompt: &Prompt,
    definitions: &[serde_json::Value],
) -> Result<Admitted, Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(crate::sqlite_error::store)?;
    let connection = &mut *transaction;
    let session = crate::sqlite_sessions::id(connection, prompt).await?;
    let lane =
        crate::sqlite_accept::admission_lane_id(connection, session, prompt, definitions).await?;
    let admitted = decide(connection, session, lane, prompt).await?;
    transaction.commit().await.map_err(crate::sqlite_error::store)?;
    Ok(admitted)
}

async fn decide(
    connection: &mut SqliteConnection,
    session: SessionId,
    lane: LaneId,
    prompt: &Prompt,
) -> Result<Admitted, Error> {
    if crate::sqlite_admit_queries::operation_exists(connection, lane, prompt).await? {
        let accepted =
            crate::sqlite_operation_existing::existing(connection, session, lane, prompt).await?;
        return Ok(accepted.into());
    }
    if let Some(existing) =
        crate::sqlite_admit_existing::existing(connection, lane, prompt).await?
    {
        return Ok(existing);
    }
    if let Some(operation) = crate::sqlite_admit_queries::active(connection, lane, prompt).await?
    {
        crate::sqlite_admit_queries::insert(connection, lane, operation, prompt).await?;
        crate::sqlite_delivery::invalidate_presentation(connection, operation).await?;
        return crate::sqlite_admit_result::steering(connection, operation, false).await;
    }
    crate::sqlite_profile::configure(connection, lane, prompt).await?;
    let accepted = crate::sqlite_operation::operation(connection, session, lane, prompt).await?;
    crate::sqlite_delivery::register(connection, accepted.operation.id, prompt).await?;
    Ok(accepted.into())
}
