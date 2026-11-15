use serde_json::Value;
use sqlx::PgConnection;
use sqlx::types::Json;

use crate::postgres_delivery_claim::Claimed;
use crate::{DeliveryBatch, DeliveryEvent, DeliveryLease, Error};

pub(crate) const INSERT: &str = "WITH inserted AS (INSERT INTO operation_events \
     (operation_id, sequence, schema_version, payload) VALUES ($1, $2, $3, $4) \
     RETURNING operation_id) UPDATE operation_deliveries d SET presented_at = NULL, \
     lease_owner = NULL, lease_expires_at = NULL, lease_token = lease_token + 1 \
     FROM inserted i WHERE d.operation_id = i.operation_id \
     AND d.next_event_sequence = $2 AND d.lease_owner IS NOT NULL";

pub(crate) async fn events(
    connection: &mut PgConnection,
    claimed: Claimed,
    limit: usize,
) -> Result<DeliveryBatch, Error> {
    let rows = event_rows(connection, &claimed, limit).await?;
    let events = rows
        .into_iter()
        .map(parse_event)
        .collect::<Result<_, _>>()?;
    Ok(DeliveryBatch {
        lease: DeliveryLease {
            operation_id: claimed.id,
            worker: claimed.worker,
            token: claimed.token,
        },
        target: claimed.target,
        external_id: claimed.external_id,
        events,
        notice: crate::schedule_failure::failure_notice(claimed.consecutive_failures),
        accepted_unix_ms: claimed.accepted_unix_ms,
    })
}

async fn event_rows(
    connection: &mut PgConnection,
    claimed: &Claimed,
    limit: usize,
) -> Result<Vec<(i64, i16, Json<Value>)>, Error> {
    let limit = i64::try_from(limit).map_err(Error::corrupt)?;
    sqlx::query_as(
        "SELECT sequence, schema_version, payload FROM operation_events \
         WHERE operation_id = $1 AND sequence >= $2 ORDER BY sequence LIMIT $3",
    )
    .bind(claimed.id.0)
    .bind(claimed.cursor)
    .bind(limit)
    .fetch_all(connection)
    .await
    .map_err(Error::store)
}

fn parse_event(row: (i64, i16, Json<Value>)) -> Result<DeliveryEvent, Error> {
    crate::stored_event::check_version(row.1)?;
    Ok(DeliveryEvent {
        sequence: row.0,
        event: serde_json::from_value(row.2.0).map_err(Error::corrupt)?,
    })
}
