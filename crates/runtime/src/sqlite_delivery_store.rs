use std::time::Duration;

use sqlx::types::Json;

use crate::{
    DeliveryBatch, DeliveryCheckpoint, DeliveryClaim, DeliveryLease, DeliveryStore, DeliveryTarget,
    Error, OperationId, SqliteStore,
};

impl DeliveryStore for SqliteStore {
    async fn claim_delivery(&self, claim: &DeliveryClaim) -> Result<Option<DeliveryBatch>, Error> {
        crate::sqlite_delivery::claim(&self.pool, claim).await
    }

    async fn checkpoint_delivery(&self, checkpoint: &DeliveryCheckpoint) -> Result<(), Error> {
        crate::sqlite_delivery_checkpoint::checkpoint(&self.pool, checkpoint).await
    }

    async fn renew_delivery(&self, lease: &DeliveryLease, ttl: Duration) -> Result<(), Error> {
        crate::sqlite_delivery::renew(&self.pool, lease, ttl).await
    }

    async fn save_delivery_stream(
        &self,
        lease: &DeliveryLease,
        external_id: Option<&str>,
    ) -> Result<(), Error> {
        crate::sqlite_delivery::save_stream(&self.pool, lease, external_id).await
    }

    async fn delivery_target(
        &self,
        operation: OperationId,
    ) -> Result<Option<DeliveryTarget>, Error> {
        let target: Option<Option<Json<DeliveryTarget>>> =
            sqlx::query_scalar("SELECT delivery_target FROM operations WHERE id = ?1")
                .bind(operation.0)
                .fetch_optional(&self.pool)
                .await
                .map_err(crate::sqlite_error::store)?;
        Ok(target.flatten().map(|Json(target)| target))
    }

    async fn bind_lane(
        &self,
        lease: &DeliveryLease,
        external_id: &str,
        target: &DeliveryTarget,
    ) -> Result<bool, Error> {
        crate::sqlite_lane_bind::bind(&self.pool, lease, external_id, target).await
    }

    async fn media(
        &self,
        operation: OperationId,
        reference: &str,
    ) -> Result<Option<crate::Media>, Error> {
        crate::sqlite_media::load(&self.pool, operation, reference).await
    }

    async fn source_text(&self, operation: OperationId) -> Result<Option<String>, Error> {
        let text: Option<Option<String>> =
            sqlx::query_scalar("SELECT source_prompt FROM operations WHERE id = ?1")
                .bind(operation.0)
                .fetch_optional(&self.pool)
                .await
                .map_err(crate::sqlite_error::store)?;
        Ok(text.flatten())
    }
}
