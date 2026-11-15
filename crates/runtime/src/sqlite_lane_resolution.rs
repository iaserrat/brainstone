use sqlx::SqliteConnection;
use sqlx::types::Json;

use crate::{Error, LaneId, Prompt, SessionId};

pub(crate) enum Mode {
    Strict,
    SurfaceAdmission,
}

pub(crate) async fn resolve(
    connection: &mut SqliteConnection,
    session_id: SessionId,
    prompt: &Prompt,
    mode: Mode,
) -> Result<LaneId, Error> {
    let (id, system, Json(profile), identity): (i64, String, Json<agent::ExecutionProfile>, Option<Json<serde_json::Value>>) = sqlx::query_as(
        "SELECT id, system_prompt, execution_profile, execution_identity FROM lanes WHERE session_id = ?1 AND external_id = ?2",
    )
    .bind(session_id.0)
    .bind(&prompt.address.lane)
    .fetch_one(connection)
    .await
    .map_err(crate::sqlite_error::store)?;
    let identity = crate::sqlite_identity::decode(identity)?;
    crate::identity_boundary::check(identity.as_ref(), prompt.identity.as_ref())?;
    if matches!(mode, Mode::Strict) && (system != prompt.system || profile != prompt.profile) {
        return Err(Error::LaneConfigurationConflict);
    }
    Ok(LaneId(id))
}
