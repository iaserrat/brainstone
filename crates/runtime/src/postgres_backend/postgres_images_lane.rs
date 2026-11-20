use crate::postgres_backend::postgres_images;
use std::collections::HashMap;

use agent::Image;
use sqlx::PgConnection;

use crate::postgres_backend::postgres_images::Row;
use crate::{Error, LaneId, OperationId};

pub(crate) async fn load(
    connection: &mut PgConnection,
    lane: LaneId,
    through: OperationId,
    cutoff: i64,
) -> Result<HashMap<i64, Vec<Image>>, Error> {
    let counts: Vec<(i64, i16)> = sqlx::query_as(
        "SELECT id, image_count FROM operations WHERE lane_id = $1 AND id <= $2 AND id > $3",
    )
    .bind(lane.0)
    .bind(through.0)
    .bind(cutoff)
    .fetch_all(&mut *connection)
    .await
    .map_err(Error::store)?;
    let rows: Vec<Row> = sqlx::query_as("SELECT i.operation_id, i.ordinal, i.media_type, i.bytes, i.width, i.height FROM operation_images i JOIN operations o ON o.id = i.operation_id WHERE o.lane_id = $1 AND o.id <= $2 AND o.id > $3 ORDER BY i.operation_id, i.ordinal")
        .bind(lane.0).bind(through.0).bind(cutoff)
        .fetch_all(connection).await.map_err(Error::store)?;
    crate::stored_attachments::group(counts, rows, |row| row.0, postgres_images::parse_group)
}
