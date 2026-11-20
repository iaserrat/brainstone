use crate::postgres_backend::{postgres_accept, postgres_identity, postgres_subagent_read};
use sqlx::PgConnection;
use sqlx::types::Json;

use crate::{Address, Error, PostgresStore, Prompt, Subagent, SubagentId, SubagentRequest};

impl PostgresStore {
    pub(crate) async fn message(
        &self,
        caller: crate::OperationId,
        id: SubagentId,
        key: &str,
        message: &str,
    ) -> Result<Subagent, Error> {
        let mut transaction = self.pool.begin().await.map_err(Error::store)?;
        postgres_subagent_read::authorize_on(&mut transaction, caller, id).await?;
        let request = request(&mut transaction, id).await?;
        let prompt = prompt(id, key, message, &request)?;
        let prompt = postgres_identity::inherit(&mut transaction, request.parent, prompt).await?;
        let accepted =
            postgres_accept::accept_in(&mut transaction, &prompt, &request.config.tools).await?;
        sqlx::query("UPDATE subagent_runs SET child_operation_id = $2 WHERE id = $1 AND NOT $3")
            .bind(id.0)
            .bind(accepted.operation.id.0)
            .bind(accepted.duplicate)
            .execute(&mut *transaction)
            .await
            .map_err(Error::store)?;
        transaction.commit().await.map_err(Error::store)?;
        Ok(Subagent {
            id,
            operation: accepted.operation,
            name: request.config.name,
            background: request.config.background,
        })
    }
}

async fn request(connection: &mut PgConnection, id: SubagentId) -> Result<SubagentRequest, Error> {
    let Json(request) =
        sqlx::query_scalar("SELECT request FROM subagent_runs WHERE id = $1 FOR UPDATE")
            .bind(id.0)
            .fetch_optional(connection)
            .await
            .map_err(Error::store)?
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
