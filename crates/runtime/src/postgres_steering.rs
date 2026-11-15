use agent::{Message, Outcome};
use sqlx::PgPool;

use crate::{Error, Lease, Settled};

pub(crate) async fn has_pending(pool: &PgPool, lease: &Lease) -> Result<bool, Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM operation_steering WHERE operation_id=o.id AND NOT consumed) \
         FROM operations o JOIN lanes l ON l.id=o.lane_id JOIN sessions s ON s.id=l.session_id \
         WHERE o.id=$1 AND s.id=$2 AND s.lease_owner=$3 AND s.lease_token=$4 \
         AND o.lease_token=$4 AND o.state='running' AND s.lease_expires_at>clock_timestamp()",
    ).bind(lease.operation_id.0).bind(lease.session_id.0).bind(&lease.worker).bind(lease.token)
        .fetch_optional(pool).await.map_err(Error::store)?.ok_or(Error::LeaseLost)
}

pub(crate) async fn drain(pool: &PgPool, lease: &Lease) -> Result<Vec<Message>, Error> {
    let mut transaction = pool.begin().await.map_err(Error::store)?;
    let connection = &mut *transaction;
    let sequence = crate::postgres_steering_write::lock(connection, lease).await?;
    let pending = crate::postgres_steering_pending::pending(connection, lease).await?;
    let messages =
        crate::postgres_steering_write::append_users(connection, lease, sequence, &pending).await?;
    transaction.commit().await.map_err(Error::store)?;
    Ok(messages)
}

pub(crate) async fn settle(
    pool: &PgPool,
    lease: &Lease,
    outcome: &Outcome,
) -> Result<Settled, Error> {
    let mut transaction = pool.begin().await.map_err(Error::store)?;
    let connection = &mut *transaction;
    let sequence = crate::postgres_steering_write::lock(connection, lease).await?;
    let pending = crate::postgres_steering_pending::pending(connection, lease).await?;
    let settled = if pending.is_empty() {
        crate::postgres_steering_write::finish(connection, lease, sequence, outcome).await?;
        Settled::Finished
    } else {
        let messages =
            crate::postgres_steering_write::append_users(connection, lease, sequence, &pending)
                .await?;
        Settled::Steered(messages)
    };
    transaction.commit().await.map_err(Error::store)?;
    Ok(settled)
}
