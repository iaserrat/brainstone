use crate::sqlite_backend::sqlite_error::store;

use sqlx::SqliteConnection;

use crate::{Error, Prompt, SessionId};

pub(crate) async fn id(
    connection: &mut SqliteConnection,
    prompt: &Prompt,
) -> Result<SessionId, Error> {
    if let Some(id) = insert(connection, prompt).await? {
        return Ok(SessionId(id));
    }
    existing(connection, prompt).await
}

async fn insert(connection: &mut SqliteConnection, prompt: &Prompt) -> Result<Option<i64>, Error> {
    sqlx::query_scalar(
        "INSERT INTO sessions (surface, external_id) VALUES (?1, ?2) \
         ON CONFLICT (surface, external_id) DO NOTHING RETURNING id",
    )
    .bind(&prompt.address.surface)
    .bind(&prompt.address.conversation)
    .fetch_optional(connection)
    .await
    .map_err(store)
}

async fn existing(connection: &mut SqliteConnection, prompt: &Prompt) -> Result<SessionId, Error> {
    sqlx::query_scalar("SELECT id FROM sessions WHERE surface = ?1 AND external_id = ?2")
        .bind(&prompt.address.surface)
        .bind(&prompt.address.conversation)
        .fetch_one(connection)
        .await
        .map(SessionId)
        .map_err(store)
}
