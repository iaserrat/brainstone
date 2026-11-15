use sqlx::SqliteConnection;
use sqlx::types::Json;

use crate::{Error, LaneId, Prompt};

pub(crate) async fn configure(
    connection: &mut SqliteConnection,
    lane: LaneId,
    prompt: &Prompt,
) -> Result<(), Error> {
    sqlx::query("UPDATE lanes SET execution_profile = ?2 WHERE id = ?1")
        .bind(lane.0)
        .bind(Json(&prompt.profile))
        .execute(connection)
        .await
        .map(|_| ())
        .map_err(crate::sqlite_error::store)
}
