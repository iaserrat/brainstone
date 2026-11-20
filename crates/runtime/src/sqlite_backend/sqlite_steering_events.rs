use crate::sqlite_backend::sqlite_error::store;
use crate::sqlite_backend::{sqlite_delivery_events, sqlite_steering_pending};
use agent::Event;
use sqlx::SqliteConnection;
use sqlx::types::Json;

use crate::{Error, Lease};

pub(crate) async fn insert(
    connection: &mut SqliteConnection,
    lease: &Lease,
    sequence: i64,
    event: &Event,
) -> Result<(), Error> {
    let payload = serde_json::to_value(event).map_err(Error::corrupt)?;
    sqlx::query(sqlite_delivery_events::INSERT)
        .bind(lease.operation_id.0)
        .bind(sequence)
        .bind(1_i16)
        .bind(Json(payload))
        .execute(connection)
        .await
        .map_err(store)?;
    Ok(())
}

pub(crate) async fn advance(
    connection: &mut SqliteConnection,
    lease: &Lease,
    next: i64,
) -> Result<(), Error> {
    sqlx::query("UPDATE operations SET next_event_sequence = ?2 WHERE id = ?1")
        .bind(lease.operation_id.0)
        .bind(next)
        .execute(connection)
        .await
        .map_err(store)?;
    Ok(())
}

pub(crate) async fn consume(
    connection: &mut SqliteConnection,
    pending: &[sqlite_steering_pending::Pending],
) -> Result<(), Error> {
    for entry in pending {
        sqlx::query("UPDATE operation_steering SET consumed = TRUE WHERE id = ?1")
            .bind(entry.id)
            .execute(&mut *connection)
            .await
            .map_err(store)?;
    }
    Ok(())
}
