use agent::Event;
use sqlx::types::Json;
use sqlx::{PgConnection, PgPool};

use crate::{Error, Lease};

pub(crate) async fn record(pool: &PgPool, lease: &Lease, event: &Event) -> Result<(), Error> {
    let mut transaction = pool.begin().await.map_err(Error::store)?;
    let connection = &mut *transaction;
    let Some(sequence) = lock_sequence(connection, lease).await? else {
        return Err(Error::LeaseLost);
    };
    insert_event(connection, lease, sequence, event).await?;
    advance_sequence(connection, lease, sequence).await?;
    if let Event::RunFinished { outcome } = event {
        crate::postgres_finish::finish(connection, lease, outcome).await?;
        crate::postgres_subagent_notify::notify(connection, lease.operation_id, outcome).await?;
    }
    transaction.commit().await.map_err(Error::store)
}

async fn lock_sequence(connection: &mut PgConnection, lease: &Lease) -> Result<Option<i64>, Error> {
    sqlx::query_scalar(
        "SELECT o.next_event_sequence FROM operations o JOIN lanes l ON l.id = o.lane_id \
         JOIN sessions s ON s.id = l.session_id WHERE o.id = $1 AND s.id = $2 \
         AND s.lease_owner = $3 AND s.lease_token = $4 AND o.lease_token = $4 \
         AND s.lease_expires_at > clock_timestamp() AND o.state = 'running' \
         FOR UPDATE OF o, s",
    )
    .bind(lease.operation_id.0)
    .bind(lease.session_id.0)
    .bind(&lease.worker)
    .bind(lease.token)
    .fetch_optional(connection)
    .await
    .map_err(Error::store)
}

async fn insert_event(
    connection: &mut PgConnection,
    lease: &Lease,
    sequence: i64,
    event: &Event,
) -> Result<(), Error> {
    let payload = serde_json::to_value(event).map_err(Error::corrupt)?;
    sqlx::query(crate::postgres_delivery_events::INSERT)
        .bind(lease.operation_id.0)
        .bind(sequence)
        .bind(crate::stored_event::version(event))
        .bind(Json(payload))
        .execute(connection)
        .await
        .map_err(Error::store)?;
    Ok(())
}

async fn advance_sequence(
    connection: &mut PgConnection,
    lease: &Lease,
    sequence: i64,
) -> Result<(), Error> {
    sqlx::query("UPDATE operations SET next_event_sequence = $2 WHERE id = $1")
        .bind(lease.operation_id.0)
        .bind(sequence + 1)
        .execute(connection)
        .await
        .map_err(Error::store)?;
    Ok(())
}
