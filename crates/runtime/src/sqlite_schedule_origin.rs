use sqlx::SqliteConnection;
use sqlx::types::Json;

use crate::schedule_destination::Creator;
use crate::{Error, LaneId, ScheduleRequest};

/// The creating operation: its lane, delivery target, identity, and whether a
/// schedule released it.
pub(crate) struct Origin {
    pub(crate) lane: i64,
    pub(crate) target: Option<Json<serde_json::Value>>,
    identity: Option<agent::ExecutionIdentity>,
    scheduled_run: bool,
}

impl Origin {
    pub(crate) fn creator(&self) -> Creator<'_> {
        Creator {
            identity: self.identity.as_ref(),
            has_target: self.target.is_some(),
            scheduled_run: self.scheduled_run,
        }
    }
}

type OriginRow = (i64, Option<Json<serde_json::Value>>, Option<Json<serde_json::Value>>, bool);

/// The creating operation, once it may create `request`: a post only as the
/// person who asked, and every schedule within its lane's and owner's limits.
pub(crate) async fn admitted(
    connection: &mut SqliteConnection,
    request: &ScheduleRequest,
) -> Result<Origin, Error> {
    let origin = load(connection, request).await?;
    if let Some(destination) = &request.destination {
        crate::schedule_destination::check(&origin.creator(), destination)?;
    }
    let owner = origin.identity.as_ref();
    crate::sqlite_schedule_admit::admit(connection, LaneId(origin.lane), request, owner).await?;
    Ok(origin)
}

async fn load(connection: &mut SqliteConnection, request: &ScheduleRequest) -> Result<Origin, Error> {
    let row: Option<OriginRow> = sqlx::query_as(
        "SELECT lane_id, delivery_target, execution_identity, schedule_id IS NOT NULL \
         FROM operations WHERE id = ?1",
    )
    .bind(request.operation_id.0)
    .fetch_optional(connection)
    .await
    .map_err(crate::sqlite_error::store)?;
    let (lane, target, identity, scheduled_run) = row.ok_or(Error::ScheduleNotFound)?;
    Ok(Origin {
        lane,
        target,
        identity: crate::sqlite_identity::decode(identity)?,
        scheduled_run,
    })
}
