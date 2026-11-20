use crate::sqlite_backend::sqlite_error::store;

use sqlx::SqliteConnection;

use crate::{DeliveryCheckpoint, Error};

pub(super) async fn invalidate(
    connection: &mut SqliteConnection,
    checkpoint: &DeliveryCheckpoint,
) -> Result<(), Error> {
    sqlx::query(
        "WITH candidate AS (SELECT d.operation_id FROM operation_deliveries d \
         JOIN operations sibling ON sibling.id = d.operation_id \
         JOIN operations terminal ON terminal.id = ?1 \
         WHERE terminal.state IN ('completed', 'failed') \
         AND terminal.next_event_sequence = ?2 + 1 AND sibling.lane_id = terminal.lane_id \
         AND sibling.state IN ('accepted', 'running') AND sibling.delivery_target->>'adapter' = \
         terminal.delivery_target->>'adapter' ORDER BY sibling.id LIMIT 1) \
         UPDATE operation_deliveries AS d SET presented_at = NULL, lease_owner = NULL, \
         lease_expires_at = NULL, lease_token = d.lease_token + 1 FROM candidate c \
         WHERE d.operation_id = c.operation_id \
         AND (d.presented_at IS NOT NULL OR d.lease_owner IS NOT NULL)",
    )
    .bind(checkpoint.lease.operation_id.0)
    .bind(checkpoint.through)
    .execute(connection)
    .await
    .map_err(store)?;
    Ok(())
}
