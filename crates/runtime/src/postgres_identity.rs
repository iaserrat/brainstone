use agent::ExecutionIdentity;
use sqlx::{PgConnection, types::Json};
use crate::{Error, OperationId, Prompt};

pub(crate) async fn load(connection: &mut PgConnection, id: OperationId)
    -> Result<Option<ExecutionIdentity>, Error> {
    let value: Option<Json<serde_json::Value>> = sqlx::query_scalar(
        "SELECT execution_identity FROM operations WHERE id = $1")
        .bind(id.0).fetch_one(connection).await.map_err(Error::store)?;
    decode(value)
}

pub(crate) fn decode(value: Option<Json<serde_json::Value>>)
    -> Result<Option<ExecutionIdentity>, Error> {
    let identity: Option<ExecutionIdentity> = value.map(|value| serde_json::from_value(value.0))
        .transpose().map_err(Error::corrupt)?;
    if identity.as_ref().is_some_and(|identity| !identity.is_valid()) {
        return Err(Error::corrupt("invalid execution identity"));
    }
    Ok(identity)
}

pub(crate) async fn inherit(connection: &mut PgConnection, parent: OperationId, prompt: Prompt)
    -> Result<Prompt, Error> {
    Ok(prompt.with_identity(load(connection, parent).await?))
}

pub(crate) async fn check(connection: &mut PgConnection, id: OperationId, prompt: &Prompt)
    -> Result<(), Error> {
    if load(connection, id).await? != prompt.identity {
        return Err(Error::IdempotencyConflict);
    }
    Ok(())
}
