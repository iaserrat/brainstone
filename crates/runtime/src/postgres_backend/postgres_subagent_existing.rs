use crate::postgres_backend::postgres_read;
use sqlx::PgConnection;
use sqlx::types::Json;

use crate::{Address, Error, Prompt, SpawnedSubagent, Subagent, SubagentId, SubagentRequest};

pub(crate) async fn get(
    connection: &mut PgConnection,
    request: &SubagentRequest,
) -> Result<Option<SpawnedSubagent>, Error> {
    let row: Option<(i64, Json<SubagentRequest>, Option<i64>)> = sqlx::query_as(
        "SELECT id, request, child_operation_id FROM subagent_runs \
         WHERE parent_operation_id = $1 AND spawn_key = $2",
    )
    .bind(request.parent.0)
    .bind(&request.key)
    .fetch_optional(&mut *connection)
    .await
    .map_err(Error::store)?;
    let Some((id, Json(stored), operation_id)) = row else {
        return Ok(None);
    };
    if &stored != request {
        return Err(Error::IdempotencyConflict);
    }
    let operation_id = operation_id.ok_or_else(|| Error::CorruptRecord {
        message: "subagent has no child operation".to_owned(),
    })?;
    let operation = load_operation(connection, operation_id).await?;
    Ok(Some(spawned(SubagentId(id), request, operation, true)))
}

async fn load_operation(connection: &mut PgConnection, id: i64) -> Result<crate::Operation, Error> {
    postgres_read::operation_on(connection, crate::OperationId(id))
        .await?
        .ok_or_else(|| Error::CorruptRecord {
            message: "subagent operation is missing".to_owned(),
        })
}

pub(crate) fn prompt(id: SubagentId, request: &SubagentRequest) -> Result<Prompt, Error> {
    let address = Address::new("subagent", &format!("agent-{}", id.0), "main")?;
    Ok(
        Prompt::new(address, "initial", &request.config.system, &request.task)?
            .with_profile(request.config.profile.clone()),
    )
}

pub(crate) fn spawned(
    id: SubagentId,
    request: &SubagentRequest,
    operation: crate::Operation,
    duplicate: bool,
) -> SpawnedSubagent {
    let agent = Subagent {
        id,
        operation,
        name: request.config.name.clone(),
        background: request.config.background,
    };
    SpawnedSubagent { agent, duplicate }
}
