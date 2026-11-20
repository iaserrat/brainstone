use crate::sqlite_backend::sqlite_error::store;

use sqlx::SqliteConnection;

use crate::{Error, OperationId};

const MAX_ACTIVE_SUBAGENTS: i64 = 20;

pub(crate) async fn root(
    connection: &mut SqliteConnection,
    parent: OperationId,
) -> Result<i64, Error> {
    let root: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT root_session_id FROM subagent_runs \
         WHERE child_session_id = l.session_id), l.session_id) \
         FROM operations o JOIN lanes l ON l.id = o.lane_id WHERE o.id = ?1",
    )
    .bind(parent.0)
    .fetch_optional(&mut *connection)
    .await
    .map_err(store)?
    .ok_or(Error::InvalidInput {
        field: "parent_operation_id",
    })?;
    Ok(root)
}

pub(crate) async fn check(connection: &mut SqliteConnection, root: i64) -> Result<(), Error> {
    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM subagent_runs s JOIN operations o ON o.id = s.child_operation_id \
         WHERE s.root_session_id = ?1 AND o.state IN ('accepted', 'running')",
    )
    .bind(root)
    .fetch_one(connection)
    .await
    .map_err(store)?;
    if active >= MAX_ACTIVE_SUBAGENTS {
        return Err(Error::AgentConcurrencyExceeded);
    }
    Ok(())
}
