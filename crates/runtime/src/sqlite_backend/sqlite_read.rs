use crate::sqlite_backend::sqlite_error::store;

use sqlx::types::Json;
use sqlx::{SqliteConnection, SqlitePool};

pub(crate) use crate::stored_operation::{parse_outcome, parse_state};
use serde_json::Value;

use crate::{Error, LaneId, Operation, OperationId, SessionId};

type OperationRow = (i64, i64, String, Option<Json<Value>>);

pub(crate) async fn operation(
    pool: &SqlitePool,
    id: OperationId,
) -> Result<Option<Operation>, Error> {
    let mut connection = pool.acquire().await.map_err(store)?;
    operation_on(&mut connection, id).await
}

pub(crate) async fn operation_on(
    connection: &mut SqliteConnection,
    id: OperationId,
) -> Result<Option<Operation>, Error> {
    let row: Option<OperationRow> = sqlx::query_as(
        "SELECT lanes.session_id, operations.lane_id, operations.state, operations.outcome \
         FROM operations JOIN lanes ON lanes.id = operations.lane_id \
         WHERE operations.id = ?1",
    )
    .bind(id.0)
    .fetch_optional(connection)
    .await
    .map_err(store)?;
    row.as_ref().map(|row| from_row(id, row)).transpose()
}

fn from_row(id: OperationId, row: &OperationRow) -> Result<Operation, Error> {
    let state = parse_state(&row.2)?;
    let outcome = parse_outcome(state, row.3.as_ref())?;
    Ok(Operation {
        id,
        session_id: SessionId(row.0),
        lane_id: LaneId(row.1),
        state,
        outcome,
    })
}
