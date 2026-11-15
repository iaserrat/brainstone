
use crate::{Address, Error, LaneStore, SqliteStore};

impl LaneStore for SqliteStore {
    async fn lane_is_followed(&self, address: &Address) -> Result<bool, Error> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM lanes l JOIN sessions s ON s.id = l.session_id \
             WHERE s.surface = ?1 AND s.external_id = ?2 AND l.external_id = ?3 \
             AND l.follow_enabled)",
        )
        .bind(&address.surface)
        .bind(&address.conversation)
        .bind(&address.lane)
        .fetch_one(&self.pool)
        .await
        .map_err(crate::sqlite_error::store)
    }

    async fn follow_lane(&self, address: &Address, request_id: &str) -> Result<(), Error> {
        crate::sqlite_lane_follow::follow(&self.pool, address, request_id).await
    }

    async fn unfollow_lane(&self, operation_id: crate::OperationId, call_key: &str) -> Result<(), Error> {
        crate::sqlite_lane_follow::unfollow(&self.pool, operation_id, call_key).await
    }

    async fn request_exists(&self, address: &Address, request_id: &str) -> Result<bool, Error> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM lanes l JOIN sessions s ON s.id = l.session_id \
             WHERE s.surface = ?1 AND s.external_id = ?2 AND l.external_id = ?3 AND \
             (EXISTS (SELECT 1 FROM operations o WHERE o.lane_id = l.id AND o.request_id = ?4) OR \
              EXISTS (SELECT 1 FROM operation_steering st WHERE st.lane_id = l.id AND st.request_id = ?4)))",
        )
        .bind(&address.surface).bind(&address.conversation).bind(&address.lane).bind(request_id)
        .fetch_one(&self.pool).await.map_err(crate::sqlite_error::store)
    }
}
