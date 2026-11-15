//! A post's run gets a lane of its own in the destination conversation.

use sqlx::PgConnection;
use sqlx::types::Json;

use crate::{Destination, Error};

/// A new lane in `destination`'s session, named `provisional` until the
/// surface binds it to the root it posts, with the origin lane's system
/// prompt and profile and the destination's principal.
// remove_me_hackyhack: the `tool_definitions` copy serves rollback only; see
// `postgres_accept::insert_lane`.
pub(crate) async fn open(
    connection: &mut PgConnection,
    origin_lane: i64,
    destination: &Destination,
    provisional: &str,
) -> Result<i64, Error> {
    sqlx::query(
        "INSERT INTO sessions (surface, external_id) VALUES ($1, $2) \
         ON CONFLICT (surface, external_id) DO NOTHING",
    )
    .bind(&destination.surface)
    .bind(&destination.conversation)
    .execute(&mut *connection)
    .await
    .map_err(Error::store)?;
    sqlx::query_scalar(
        "INSERT INTO lanes (session_id, external_id, system_prompt, tool_definitions, \
         execution_profile, execution_identity) SELECT s.id, $3, l.system_prompt, \
         l.tool_definitions, l.execution_profile, $4 FROM sessions s, lanes l \
         WHERE s.surface = $1 AND s.external_id = $2 AND l.id = $5 RETURNING id",
    )
    .bind(&destination.surface)
    .bind(&destination.conversation)
    .bind(provisional)
    .bind(Json(&destination.identity))
    .bind(origin_lane)
    .fetch_one(connection)
    .await
    .map_err(Error::store)
}
