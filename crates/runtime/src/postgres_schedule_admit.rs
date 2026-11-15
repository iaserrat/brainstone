use agent::ExecutionIdentity;
use sqlx::PgConnection;

use crate::schedule_owner::{MAX_POSTS_PER_OWNER_PER_HOUR, MAX_REPEAT_PER_OWNER};
use crate::schedule_repeat::MAX_REPEAT_PER_LANE;
use crate::{Error, LaneId, ScheduleRequest, ScheduleTime};

/// A new standing row fits under its lane's and its owner's caps, and a new
/// post under its owner's hourly rate. The lane lock and the owner's advisory
/// lock serialize each count with concurrent inserts across replicas.
pub(crate) async fn admit(
    connection: &mut PgConnection,
    lane: LaneId,
    request: &ScheduleRequest,
    owner: Option<&ExecutionIdentity>,
) -> Result<(), Error> {
    let standing = matches!(request.when, ScheduleTime::Repeat(_));
    if standing {
        in_lane(connection, lane).await?;
    }
    let post = request.destination.is_some();
    match owner {
        Some(owner) if standing || post => owned(connection, owner, standing, post).await,
        _ => Ok(()),
    }
}

async fn in_lane(connection: &mut PgConnection, lane: LaneId) -> Result<(), Error> {
    crate::postgres_lanes::lock(connection, lane).await?;
    let standing: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM scheduled_work \
         WHERE lane_id = $1 AND schedule_kind IN ('repeat', 'recur') AND state = 'pending'",
    )
    .bind(lane.0)
    .fetch_one(connection)
    .await
    .map_err(Error::store)?;
    if standing >= MAX_REPEAT_PER_LANE {
        return Err(Error::ScheduleLimitExceeded);
    }
    Ok(())
}

/// The owner's advisory lock key: its provider, tenant, and actor as JSON.
fn owner_key(owner: &ExecutionIdentity) -> String {
    serde_json::json!([owner.provider, owner.tenant, owner.actor]).to_string()
}

/// Its own statement: a count in the same statement would read a snapshot
/// taken before the lock was granted.
async fn lock(connection: &mut PgConnection, owner: &ExecutionIdentity) -> Result<(), Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(owner_key(owner))
        .execute(connection)
        .await
        .map_err(Error::store)?;
    Ok(())
}

/// The owner's counts, read once its advisory lock is held.
async fn owned(
    connection: &mut PgConnection,
    owner: &ExecutionIdentity,
    standing: bool,
    post: bool,
) -> Result<(), Error> {
    lock(connection, owner).await?;
    let (standing_count, recent_posts): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE s.schedule_kind IN ('repeat', 'recur') AND s.state = 'pending'), \
         count(*) FILTER (WHERE s.destination IS NOT NULL \
         AND s.created_at > transaction_timestamp() - INTERVAL '1 hour') \
         FROM scheduled_work s JOIN operations o ON o.id = s.origin_operation_id \
         WHERE o.execution_identity->>'provider' = $1 AND o.execution_identity->>'tenant' = $2 \
         AND o.execution_identity->>'actor' = $3",
    )
    .bind(&owner.provider)
    .bind(&owner.tenant)
    .bind(&owner.actor)
    .fetch_one(connection)
    .await
    .map_err(Error::store)?;
    if standing && standing_count >= MAX_REPEAT_PER_OWNER {
        return Err(Error::ScheduleOwnerLimitExceeded);
    }
    if post && recent_posts >= MAX_POSTS_PER_OWNER_PER_HOUR {
        return Err(Error::SchedulePostRateExceeded);
    }
    Ok(())
}
