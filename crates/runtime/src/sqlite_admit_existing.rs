//! Idempotent re-admission against a stored steering row.

use sqlx::SqliteConnection;
use sqlx::types::Json;

use crate::{Admitted, Error, LaneId, OperationId, Prompt, PromptContent};

type SteeringRow = (
    i64,
    i64,
    String,
    String,
    Option<Json<serde_json::Value>>,
    i16,
    Option<Json<serde_json::Value>>,
);

pub(crate) async fn existing(
    connection: &mut SqliteConnection,
    lane: LaneId,
    prompt: &Prompt,
) -> Result<Option<Admitted>, Error> {
    let row = row(connection, lane, prompt).await?;
    let Some((steering, id, text, source, target, image_count, identity)) = row
    else {
        return Ok(None);
    };
    if crate::sqlite_identity::decode(identity)? != prompt.identity {
        return Err(Error::IdempotencyConflict);
    }
    let stored = PromptContent {
        text,
        images: crate::sqlite_steering_images::load(connection, steering, image_count).await?,
    };
    crate::stored_prompt::check(prompt, &stored, &source, target.as_ref())?;
    crate::sqlite_admit_result::steering(connection, OperationId(id), true)
        .await
        .map(Some)
}

async fn row(
    connection: &mut SqliteConnection,
    lane: LaneId,
    prompt: &Prompt,
) -> Result<Option<SteeringRow>, Error> {
    sqlx::query_as(
        "SELECT id, operation_id, prompt, COALESCE(source_prompt, prompt), delivery_target, \
         image_count, execution_identity FROM operation_steering \
         WHERE lane_id = ?1 AND request_id = ?2",
    )
    .bind(lane.0)
    .bind(&prompt.request_id)
    .fetch_optional(&mut *connection)
    .await
    .map_err(crate::sqlite_error::store)
}
