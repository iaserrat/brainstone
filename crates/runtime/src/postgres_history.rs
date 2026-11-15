use agent::Thread;
use serde_json::Value;
use sqlx::PgConnection;
use sqlx::types::Json;

use crate::{Error, LaneId, OperationId};

#[path = "postgres_history_queries.rs"]
mod queries;
use queries::{configuration, compaction, rows};

pub(crate) type HistoryRow = (i64, i64, Option<String>, Option<Json<Value>>, i16);

pub(crate) async fn load(
    connection: &mut PgConnection,
    lane_id: LaneId,
    through: OperationId,
) -> Result<Thread, Error> {
    let span = agent::observation::span(agent::observation::Kind::Recovery);
    span.record("operation_id", through.get());
    agent::observation::run(span, restore(connection, lane_id, through), agent::observation::result_outcome).await
}

async fn restore(connection: &mut PgConnection, lane_id: LaneId, through: OperationId) -> Result<Thread, Error> {
    let (system, profile) = configuration(connection, lane_id).await?;
    let compaction = compaction(connection, lane_id, through).await?;
    let cutoff = compaction.as_ref().map_or(0, |row| row.0);
    let boundary =
        crate::postgres_history_boundary::latest(connection, lane_id, through, cutoff).await?;
    let rows = rows(connection, lane_id, through, boundary).await?;
    let images =
        crate::postgres_images_lane::load(connection, lane_id, through, boundary.operation).await?;
    let summary = compaction.map(|row| row.1);
    let attachments = crate::stored_history::Attachments { images: &images };
    let (messages, steps) = crate::stored_history::reduce(rows, &attachments, summary)?;
    Thread::restore_compacted(system, messages, profile, steps).ok_or_else(|| {
        Error::CorruptRecord {
            message: "claimed lane has no messages".to_owned(),
        }
    })
}
