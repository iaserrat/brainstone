use sqlx::SqlitePool;
use crate::{Address, Error, OperationId};

pub(crate) async fn follow(pool: &SqlitePool, address: &Address, request: &str) -> Result<(), Error> {
    sqlx::query("INSERT INTO lane_follow_activations (lane_id,request_id) \
        SELECT l.id,?4 FROM lanes l JOIN sessions s ON s.id=l.session_id \
        WHERE s.surface=?1 AND s.external_id=?2 AND l.external_id=?3 ON CONFLICT DO NOTHING")
        .bind(&address.surface).bind(&address.conversation).bind(&address.lane).bind(request)
        .execute(pool).await.map_err(crate::sqlite_error::store)?;
    Ok(())
}

pub(crate) async fn unfollow(pool: &SqlitePool, operation: OperationId, key: &str) -> Result<(), Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(crate::sqlite_error::store)?;
    sqlx::query("INSERT INTO lane_unfollow_calls (call_key,lane_id) \
        SELECT ?2,lane_id FROM operations WHERE id=?1 ON CONFLICT DO NOTHING")
        .bind(operation.0).bind(key).execute(&mut *transaction).await.map_err(crate::sqlite_error::store)?;
    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM lane_unfollow_calls c \
        JOIN operations o ON o.lane_id=c.lane_id WHERE c.call_key=?2 AND o.id=?1)")
        .bind(operation.0).bind(key).fetch_one(&mut *transaction).await.map_err(crate::sqlite_error::store)?;
    if !valid { return Err(Error::InvalidInput { field: "operation_id" }); }
    transaction.commit().await.map_err(crate::sqlite_error::store)
}
