use sqlx::{PgConnection, PgPool};

use crate::{Compaction, Error, OperationState};

pub(crate) async fn compact(pool: &PgPool, compaction: &Compaction) -> Result<(), Error> {
    let mut transaction = pool.begin().await.map_err(Error::store)?;
    let connection = &mut *transaction;
    validate_boundary(connection, compaction).await?;
    if !already_stored(connection, compaction).await? {
        insert(connection, compaction).await?;
    }
    transaction.commit().await.map_err(Error::store)
}

async fn validate_boundary(
    connection: &mut PgConnection,
    compaction: &Compaction,
) -> Result<(), Error> {
    let row: Option<(String, bool)> = sqlx::query_as(
        "SELECT o.state, EXISTS (SELECT 1 FROM operations active \
         WHERE active.lane_id = l.id AND active.state = 'running') \
         FROM operations o JOIN lanes l ON l.id = o.lane_id \
         WHERE o.id = $1 AND l.id = $2 FOR UPDATE OF o, l",
    )
    .bind(compaction.through.0)
    .bind(compaction.lane_id.0)
    .fetch_optional(connection)
    .await
    .map_err(Error::store)?;
    let Some((state, running)) = row else {
        return Err(Error::InvalidCompaction);
    };
    let state = crate::postgres_read::parse_state(&state)?;
    if running || !terminal(state) {
        return Err(Error::InvalidCompaction);
    }
    Ok(())
}

async fn already_stored(
    connection: &mut PgConnection,
    compaction: &Compaction,
) -> Result<bool, Error> {
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT through_operation_id, summary FROM lane_compactions \
         WHERE lane_id = $1 ORDER BY id DESC LIMIT 1",
    )
    .bind(compaction.lane_id.0)
    .fetch_optional(connection)
    .await
    .map_err(Error::store)?;
    let Some((through, summary)) = row else {
        return Ok(false);
    };
    if through > compaction.through.0
        || (through == compaction.through.0 && summary != compaction.summary)
    {
        return Err(Error::InvalidCompaction);
    }
    Ok(through == compaction.through.0)
}

async fn insert(connection: &mut PgConnection, compaction: &Compaction) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO lane_compactions (lane_id, through_operation_id, summary) \
         VALUES ($1, $2, $3)",
    )
    .bind(compaction.lane_id.0)
    .bind(compaction.through.0)
    .bind(&compaction.summary)
    .execute(connection)
    .await
    .map_err(Error::store)?;
    Ok(())
}

fn terminal(state: OperationState) -> bool {
    matches!(state, OperationState::Completed | OperationState::Failed)
}
