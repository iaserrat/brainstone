use crate::postgres_backend::{postgres_images, postgres_lanes, postgres_operation_existing};
use sqlx::PgConnection;
use sqlx::types::Json;

use crate::{Accepted, Error, LaneId, Operation, OperationId, OperationState, Prompt, SessionId};

pub(crate) async fn operation(
    connection: &mut PgConnection,
    session_id: SessionId,
    lane_id: LaneId,
    prompt: &Prompt,
) -> Result<Accepted, Error> {
    postgres_lanes::lock(connection, lane_id).await?;
    let inserted = insert(connection, lane_id, prompt).await?;
    let Some(id) = inserted else {
        return postgres_operation_existing::existing(connection, session_id, lane_id, prompt)
            .await;
    };
    let id = OperationId(id);
    postgres_images::insert(connection, id, &prompt.content.images).await?;
    Ok(Accepted::fresh(Operation {
        id,
        session_id,
        lane_id,
        state: OperationState::Accepted,
        outcome: None,
    }))
}

async fn insert(
    connection: &mut PgConnection,
    lane_id: LaneId,
    prompt: &Prompt,
) -> Result<Option<i64>, Error> {
    sqlx::query_scalar(
        "INSERT INTO operations (lane_id, request_id, prompt, source_prompt, state, delivery_target, actor, image_count, execution_identity) \
         VALUES ($1, $2, $3, $4, 'accepted', $5, $6, $7, $8) \
         ON CONFLICT (lane_id, request_id) DO NOTHING RETURNING id",
    )
    .bind(lane_id.0)
    .bind(&prompt.request_id)
    .bind(&prompt.content.text)
    .bind(&prompt.source_text)
    .bind(prompt.delivery.as_ref().map(Json))
    .bind(&prompt.actor)
    .bind(i16::try_from(prompt.content.images.len()).map_err(Error::corrupt)?)
    .bind(prompt.identity.as_ref().map(Json))
    .fetch_optional(connection)
    .await
    .map_err(Error::store)
}
