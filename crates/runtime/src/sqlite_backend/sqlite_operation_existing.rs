use crate::sqlite_backend::sqlite_error::store;
use crate::sqlite_backend::{sqlite_identity, sqlite_images, sqlite_read};
use serde_json::Value;
use sqlx::SqliteConnection;
use sqlx::types::Json;

use crate::{Accepted, Error, LaneId, Operation, OperationId, Prompt, PromptContent, SessionId};

type Row = (
    i64,
    String,
    String,
    String,
    Option<Json<Value>>,
    Option<Json<Value>>,
    i16,
);

pub(crate) async fn existing(
    connection: &mut SqliteConnection,
    session_id: SessionId,
    lane_id: LaneId,
    prompt: &Prompt,
) -> Result<Accepted, Error> {
    let row: Row = sqlx::query_as(
        "SELECT id, prompt, COALESCE(source_prompt, prompt), state, outcome, delivery_target, \
         image_count FROM operations \
         WHERE lane_id = ?1 AND request_id = ?2",
    )
    .bind(lane_id.0)
    .bind(&prompt.request_id)
    .fetch_one(&mut *connection)
    .await
    .map_err(store)?;
    sqlite_identity::check(connection, OperationId(row.0), prompt).await?;
    let stored = PromptContent {
        text: row.1.clone(),
        images: sqlite_images::load(connection, OperationId(row.0), row.6).await?,
    };
    crate::stored_prompt::check(prompt, &stored, &row.2, row.5.as_ref())?;
    accepted(&row, session_id, lane_id)
}

fn accepted(row: &Row, session_id: SessionId, lane_id: LaneId) -> Result<Accepted, Error> {
    let state = sqlite_read::parse_state(&row.3)?;
    let outcome = sqlite_read::parse_outcome(state, row.4.as_ref())?;
    Ok(Accepted {
        operation: Operation {
            id: OperationId(row.0),
            session_id,
            lane_id,
            state,
            outcome,
        },
        duplicate: true,
    })
}
