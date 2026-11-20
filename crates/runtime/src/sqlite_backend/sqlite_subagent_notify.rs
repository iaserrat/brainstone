use crate::sqlite_backend::sqlite_error::store;

use agent::Outcome;
use sqlx::SqliteConnection;
use sqlx::types::Json;

use crate::{Error, OperationId};

type Parent = (i64, i64, String, Json<serde_json::Value>, i64);

pub(crate) async fn notify(
    connection: &mut SqliteConnection,
    child: OperationId,
    outcome: &Outcome,
) -> Result<(), Error> {
    let Some(parent) = parent(connection, child).await? else {
        return Ok(());
    };
    let request_id = format!("subagent-complete/{}/{}", parent.0, child.0);
    let prompt = serde_json::json!({
        "type":"subagent_completion", "agent_id":parent.0,
        "agent_name":parent.2, "outcome":outcome,
    })
    .to_string();
    let operation = insert(connection, &request_id, &prompt, &parent).await?;
    sqlx::query(
        "INSERT INTO operation_deliveries (operation_id) SELECT ?1 WHERE ?2 <> 'null' \
         ON CONFLICT DO NOTHING",
    )
    .bind(operation)
    .bind(&parent.3)
    .execute(connection)
    .await
    .map_err(store)?;
    Ok(())
}

async fn insert(
    connection: &mut SqliteConnection,
    request_id: &str,
    prompt: &str,
    parent: &Parent,
) -> Result<i64, Error> {
    sqlx::query_scalar(
        "INSERT INTO operations (lane_id, request_id, prompt, source_prompt, state, delivery_target, image_count, execution_identity) \
         VALUES (?1, ?2, ?3, ?3, 'accepted', ?4, 0, (SELECT execution_identity FROM operations WHERE id = ?5)) ON CONFLICT (lane_id, request_id) \
         DO UPDATE SET request_id = EXCLUDED.request_id RETURNING id",
    )
    .bind(parent.1)
    .bind(request_id)
    .bind(prompt)
    .bind(&parent.3)
    .bind(parent.4)
    .fetch_one(&mut *connection)
    .await
    .map_err(store)
}

async fn parent(
    connection: &mut SqliteConnection,
    child: OperationId,
) -> Result<Option<Parent>, Error> {
    sqlx::query_as(
        "SELECT s.id, parent.lane_id, s.request->'config'->>'name', \
         COALESCE(parent.delivery_target, 'null'), parent.id FROM subagent_runs s \
         JOIN operations parent ON parent.id = s.parent_operation_id \
         WHERE s.child_operation_id = ?1 AND (s.request->'config'->>'background')",
    )
    .bind(child.0)
    .fetch_optional(connection)
    .await
    .map_err(store)
}
