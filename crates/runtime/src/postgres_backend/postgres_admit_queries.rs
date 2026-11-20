use crate::postgres_backend::postgres_steering_images;
use sqlx::PgConnection;
use sqlx::types::Json;

use crate::{Error, LaneId, OperationId, Prompt};

pub(crate) async fn operation_exists(
    connection: &mut PgConnection,
    lane: LaneId,
    prompt: &Prompt,
) -> Result<bool, Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM operations WHERE lane_id = $1 AND request_id = $2)",
    )
    .bind(lane.0)
    .bind(&prompt.request_id)
    .fetch_one(connection)
    .await
    .map_err(Error::store)
}

pub(crate) async fn active(
    connection: &mut PgConnection,
    lane: LaneId,
    prompt: &Prompt,
) -> Result<Option<OperationId>, Error> {
    sqlx::query_scalar("SELECT id FROM operations WHERE lane_id = $1 AND state = 'running' AND execution_identity IS NOT DISTINCT FROM $2 LIMIT 1")
        .bind(lane.0)
        .bind(prompt.identity.as_ref().map(Json))
        .fetch_optional(connection)
        .await
        .map(|value| value.map(OperationId))
        .map_err(Error::store)
}

pub(crate) async fn insert(
    connection: &mut PgConnection,
    lane: LaneId,
    operation: OperationId,
    prompt: &Prompt,
) -> Result<(), Error> {
    let steering: i64 = sqlx::query_scalar("INSERT INTO operation_steering (lane_id, operation_id, request_id, prompt, source_prompt, delivery_target, image_count, execution_identity) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id")
        .bind(lane.0).bind(operation.0).bind(&prompt.request_id).bind(&prompt.content.text)
        .bind(&prompt.source_text).bind(prompt.delivery.as_ref().map(Json))
        .bind(i16::try_from(prompt.content.images.len()).map_err(Error::corrupt)?)
        .bind(prompt.identity.as_ref().map(Json))
        .fetch_one(&mut *connection).await.map_err(Error::store)?;
    postgres_steering_images::insert(connection, steering, &prompt.content.images).await?;
    Ok(())
}
