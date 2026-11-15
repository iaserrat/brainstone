use sqlx::{PgConnection, PgPool};

use crate::{DeliveryBatch, DeliveryCheckpoint, DeliveryClaim, Error, OperationId, Prompt};

#[path = "postgres_delivery_lease.rs"]
mod lease;

pub(crate) use lease::renew;

pub(crate) async fn register(
    connection: &mut PgConnection,
    id: OperationId,
    prompt: &Prompt,
) -> Result<(), Error> {
    if prompt.delivery.is_none() {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO operation_deliveries (operation_id) VALUES ($1) ON CONFLICT DO NOTHING",
    )
    .bind(id.0)
    .execute(connection)
    .await
    .map_err(Error::store)?;
    Ok(())
}

pub(crate) async fn invalidate_presentation(
    connection: &mut PgConnection,
    id: OperationId,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE operation_deliveries SET presented_at = NULL, lease_owner = NULL, \
         lease_expires_at = NULL, lease_token = lease_token + 1 WHERE operation_id = $1",
    )
    .bind(id.0)
    .execute(connection)
    .await
    .map_err(Error::store)?;
    Ok(())
}

pub(crate) async fn claim(
    pool: &PgPool,
    claim: &DeliveryClaim,
) -> Result<Option<DeliveryBatch>, Error> {
    let mut transaction = pool.begin().await.map_err(Error::store)?;
    let claimed = crate::postgres_delivery_claim::claim(&mut transaction, claim).await?;
    let Some(claimed) = claimed else {
        return Ok(None);
    };
    let batch =
        crate::postgres_delivery_events::events(&mut transaction, claimed, claim.limit).await?;
    transaction.commit().await.map_err(Error::store)?;
    Ok(Some(batch))
}

pub(crate) async fn checkpoint(
    pool: &PgPool,
    checkpoint: &DeliveryCheckpoint,
) -> Result<(), Error> {
    crate::postgres_delivery_checkpoint::checkpoint(pool, checkpoint).await
}

pub(crate) async fn save_stream(
    pool: &PgPool,
    lease: &crate::DeliveryLease,
    external_id: Option<&str>,
) -> Result<(), Error> {
    let updated = sqlx::query(
        "UPDATE operation_deliveries SET external_id = $3 WHERE operation_id = $1 \
         AND lease_owner = $2 AND lease_token = $4 AND lease_expires_at > clock_timestamp()",
    )
    .bind(lease.operation_id.0)
    .bind(&lease.worker)
    .bind(external_id)
    .bind(lease.token)
    .execute(pool)
    .await
    .map_err(Error::store)?;
    if updated.rows_affected() == 0 {
        return Err(Error::LeaseLost);
    }
    Ok(())
}
