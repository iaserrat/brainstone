use crate::postgres_backend::{
    postgres_accept, postgres_admit_existing, postgres_admit_queries, postgres_admit_result,
    postgres_delivery, postgres_lanes, postgres_operation, postgres_operation_existing,
    postgres_profile, postgres_sessions,
};
use sqlx::{PgConnection, PgPool};

use crate::{Admitted, Error, LaneId, Prompt, SessionId};

pub(crate) async fn admit(
    pool: &PgPool,
    prompt: &Prompt,
    definitions: &[serde_json::Value],
) -> Result<Admitted, Error> {
    let mut transaction = pool.begin().await.map_err(Error::store)?;
    let connection = &mut *transaction;
    let session = postgres_sessions::id(connection, prompt).await?;
    let lane = postgres_accept::admission_lane_id(connection, session, prompt, definitions).await?;
    postgres_lanes::lock(connection, lane).await?;
    let admitted = decide(connection, session, lane, prompt).await?;
    transaction.commit().await.map_err(Error::store)?;
    Ok(admitted)
}

async fn decide(
    connection: &mut PgConnection,
    session: SessionId,
    lane: LaneId,
    prompt: &Prompt,
) -> Result<Admitted, Error> {
    if postgres_admit_queries::operation_exists(connection, lane, prompt).await? {
        let accepted =
            postgres_operation_existing::existing(connection, session, lane, prompt).await?;
        return Ok(accepted.into());
    }
    if let Some(existing) = postgres_admit_existing::existing(connection, lane, prompt).await? {
        return Ok(existing);
    }
    if let Some(operation) = postgres_admit_queries::active(connection, lane, prompt).await? {
        postgres_admit_queries::insert(connection, lane, operation, prompt).await?;
        postgres_delivery::invalidate_presentation(connection, operation).await?;
        return postgres_admit_result::steering(connection, operation, false).await;
    }
    postgres_profile::configure(connection, lane, prompt).await?;
    let accepted = postgres_operation::operation(connection, session, lane, prompt).await?;
    postgres_delivery::register(connection, accepted.operation.id, prompt).await?;
    Ok(accepted.into())
}
