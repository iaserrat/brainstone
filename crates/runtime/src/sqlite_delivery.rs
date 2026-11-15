use sqlx::{SqliteConnection, SqlitePool};

use crate::{DeliveryBatch, DeliveryClaim, Error, OperationId, Prompt};

#[path = "sqlite_delivery_lease.rs"]
mod lease;

pub(crate) use lease::renew;

pub(crate) async fn register(
    connection: &mut SqliteConnection,
    id: OperationId,
    prompt: &Prompt,
) -> Result<(), Error> {
    if prompt.delivery.is_none() {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO operation_deliveries (operation_id) VALUES (?1) ON CONFLICT DO NOTHING",
    )
    .bind(id.0)
    .execute(connection)
    .await
    .map_err(crate::sqlite_error::store)?;
    Ok(())
}

pub(crate) async fn invalidate_presentation(
    connection: &mut SqliteConnection,
    id: OperationId,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE operation_deliveries SET presented_at = NULL, lease_owner = NULL, \
         lease_expires_at = NULL, lease_token = lease_token + 1 WHERE operation_id = ?1",
    )
    .bind(id.0)
    .execute(connection)
    .await
    .map_err(crate::sqlite_error::store)?;
    Ok(())
}

pub(crate) async fn claim(
    pool: &SqlitePool,
    claim: &DeliveryClaim,
) -> Result<Option<DeliveryBatch>, Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(crate::sqlite_error::store)?;
    let claimed = crate::sqlite_delivery_claim::claim(&mut transaction, claim).await?;
    let Some(claimed) = claimed else {
        return Ok(None);
    };
    let batch =
        crate::sqlite_delivery_events::events(&mut transaction, claimed, claim.limit).await?;
    transaction.commit().await.map_err(crate::sqlite_error::store)?;
    Ok(Some(batch))
}

pub(crate) async fn save_stream(
    pool: &SqlitePool,
    lease: &crate::DeliveryLease,
    external_id: Option<&str>,
) -> Result<(), Error> {
    let updated = sqlx::query(
        "UPDATE operation_deliveries SET external_id = ?3 WHERE operation_id = ?1 \
         AND lease_owner = ?2 AND lease_token = ?4 AND lease_expires_at > CAST(unixepoch('subsec') * 1000 AS INTEGER)",
    )
    .bind(lease.operation_id.0)
    .bind(&lease.worker)
    .bind(external_id)
    .bind(lease.token)
    .execute(pool)
    .await
    .map_err(crate::sqlite_error::store)?;
    if updated.rows_affected() == 0 {
        return Err(Error::LeaseLost);
    }
    Ok(())
}
