use sqlx::PgConnection;

use crate::{Address, Error, LaneId, LaneStore, PostgresStore};

pub(crate) async fn lock(connection: &mut PgConnection, lane: LaneId) -> Result<(), Error> {
    sqlx::query("SELECT id FROM lanes WHERE id = $1 FOR UPDATE")
        .bind(lane.0)
        .execute(connection)
        .await
        .map_err(Error::store)?;
    Ok(())
}

impl LaneStore for PostgresStore {
    async fn lane_is_followed(&self, address: &Address) -> Result<bool, Error> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM lanes l JOIN sessions s ON s.id = l.session_id \
             WHERE s.surface = $1 AND s.external_id = $2 AND l.external_id = $3 \
             AND l.follow_enabled)",
        )
        .bind(&address.surface)
        .bind(&address.conversation)
        .bind(&address.lane)
        .fetch_one(&self.pool)
        .await
        .map_err(Error::store)
    }

    async fn follow_lane(&self, address: &Address, request_id: &str) -> Result<(), Error> {
        sqlx::query(
            "WITH lane AS (SELECT l.id FROM lanes l JOIN sessions s ON s.id = l.session_id \
             WHERE s.surface = $1 AND s.external_id = $2 AND l.external_id = $3), \
             activated AS (INSERT INTO lane_follow_activations (lane_id, request_id) \
             SELECT id, $4 FROM lane ON CONFLICT DO NOTHING RETURNING lane_id) \
             UPDATE lanes SET follow_enabled = TRUE WHERE id IN (SELECT lane_id FROM activated)",
        )
        .bind(&address.surface)
        .bind(&address.conversation)
        .bind(&address.lane)
        .bind(request_id)
        .execute(&self.pool)
        .await
        .map_err(Error::store)?;
        Ok(())
    }

    async fn unfollow_lane(
        &self,
        operation_id: crate::OperationId,
        call_key: &str,
    ) -> Result<(), Error> {
        let valid: bool = sqlx::query_scalar(
            "WITH target AS (SELECT lane_id FROM operations WHERE id = $1), \
             recorded AS (INSERT INTO lane_unfollow_calls (call_key, lane_id) \
             SELECT $2, lane_id FROM target ON CONFLICT DO NOTHING RETURNING lane_id), \
             changed AS (UPDATE lanes SET follow_enabled = FALSE \
             WHERE id IN (SELECT lane_id FROM recorded) RETURNING id) \
             SELECT EXISTS (SELECT 1 FROM recorded) OR EXISTS \
             (SELECT 1 FROM target t JOIN lane_unfollow_calls c ON c.call_key = $2 \
             WHERE c.lane_id = t.lane_id)",
        )
        .bind(operation_id.0)
        .bind(call_key)
        .fetch_one(&self.pool)
        .await
        .map_err(Error::store)?;
        valid.then_some(()).ok_or(Error::InvalidInput {
            field: "operation_id",
        })
    }

    async fn request_exists(&self, address: &Address, request_id: &str) -> Result<bool, Error> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM lanes l JOIN sessions s ON s.id = l.session_id \
             WHERE s.surface = $1 AND s.external_id = $2 AND l.external_id = $3 AND \
             (EXISTS (SELECT 1 FROM operations o WHERE o.lane_id = l.id AND o.request_id = $4) OR \
              EXISTS (SELECT 1 FROM operation_steering st WHERE st.lane_id = l.id AND st.request_id = $4)))",
        )
        .bind(&address.surface).bind(&address.conversation).bind(&address.lane).bind(request_id)
        .fetch_one(&self.pool).await.map_err(Error::store)
    }
}
