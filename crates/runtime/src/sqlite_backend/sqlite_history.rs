use crate::sqlite_backend::{sqlite_history_boundary, sqlite_images_lane};
use agent::Thread;
use serde_json::Value;
use sqlx::SqliteConnection;
use sqlx::types::Json;

use crate::{Error, LaneId, OperationId};

#[path = "sqlite_history_queries.rs"]
mod queries;
use queries::{compaction, configuration, rows};

pub(crate) type HistoryRow = (i64, i64, Option<String>, Option<Json<Value>>, i16);

pub(crate) async fn load(
    connection: &mut SqliteConnection,
    lane_id: LaneId,
    through: OperationId,
) -> Result<Thread, Error> {
    let span = agent::observation::span(agent::observation::Kind::Recovery);
    span.record("operation_id", through.get());
    agent::observation::run(
        span,
        restore(connection, lane_id, through),
        agent::observation::result_outcome,
    )
    .await
}

async fn restore(
    connection: &mut SqliteConnection,
    lane_id: LaneId,
    through: OperationId,
) -> Result<Thread, Error> {
    let (system, profile) = configuration(connection, lane_id).await?;
    let compaction = compaction(connection, lane_id, through).await?;
    let cutoff = compaction.as_ref().map_or(0, |row| row.0);
    let boundary = sqlite_history_boundary::latest(connection, lane_id, through, cutoff).await?;
    let rows = rows(connection, lane_id, through, boundary).await?;
    let images = sqlite_images_lane::load(connection, lane_id, through, boundary.operation).await?;
    let summary = compaction.map(|row| row.1);
    let attachments = crate::stored_history::Attachments { images: &images };
    let (messages, steps) = crate::stored_history::reduce(rows, &attachments, summary)?;
    let required = profile
        .tool_contract()
        .is_some_and(|contract| !contract.required_tools().is_empty());
    let evidence = queries::tool_evidence(connection, through, required).await?;
    Thread::restore_compacted(system, messages, profile, steps)
        .map(|thread| evidence.restore(thread))
        .ok_or_else(|| Error::CorruptRecord {
            message: "claimed lane has no messages".to_owned(),
        })
}
