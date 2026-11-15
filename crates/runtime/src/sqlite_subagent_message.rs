use sqlx::SqliteConnection;
use sqlx::types::Json;

use crate::{Address, Error, SqliteStore, Prompt, Subagent, SubagentId, SubagentRequest};

impl SqliteStore {
    pub(crate) async fn message(
        &self,
        caller: crate::OperationId,
        id: SubagentId,
        key: &str,
        message: &str,
    ) -> Result<Subagent, Error> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await.map_err(crate::sqlite_error::store)?;
        crate::sqlite_subagent_read::authorize_on(&mut transaction, caller, id).await?;
        let request = request(&mut transaction, id).await?;
        let prompt = prompt(id, key, message, &request)?;
        let prompt =
            crate::sqlite_identity::inherit(&mut transaction, request.parent, prompt).await?;
        let accepted =
            crate::sqlite_accept::accept_in(&mut transaction, &prompt, &request.config.tools)
                .await?;
        sqlx::query("UPDATE subagent_runs SET child_operation_id = ?2 WHERE id = ?1 AND NOT ?3")
            .bind(id.0)
            .bind(accepted.operation.id.0)
            .bind(accepted.duplicate)
            .execute(&mut *transaction)
            .await
            .map_err(crate::sqlite_error::store)?;
        transaction.commit().await.map_err(crate::sqlite_error::store)?;
        Ok(Subagent {
            id,
            operation: accepted.operation,
            name: request.config.name,
            background: request.config.background,
        })
    }
}

async fn request(connection: &mut SqliteConnection, id: SubagentId) -> Result<SubagentRequest, Error> {
    let Json(request) =
        sqlx::query_scalar("SELECT request FROM subagent_runs WHERE id = ?1")
            .bind(id.0)
            .fetch_optional(connection)
            .await
            .map_err(crate::sqlite_error::store)?
            .ok_or(Error::InvalidInput { field: "agent_id" })?;
    Ok(request)
}

fn prompt(
    id: SubagentId,
    key: &str,
    message: &str,
    request: &SubagentRequest,
) -> Result<Prompt, Error> {
    let address = Address::new("subagent", &format!("agent-{}", id.0), "main")?;
    Ok(Prompt::new(address, key, &request.config.system, message)?
        .with_profile(request.config.profile.clone()))
}
