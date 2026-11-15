use sqlx::PgConnection;

use crate::{Error, LaneId, OperationId};

#[derive(Clone, Copy)]
pub(crate) struct Boundary {
    pub(crate) operation: i64,
    pub(crate) sequence: i64,
}

pub(crate) async fn latest(
    connection: &mut PgConnection,
    lane: LaneId,
    through: OperationId,
    cutoff: i64,
) -> Result<Boundary, Error> {
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT e.operation_id, e.sequence FROM operation_events e \
         JOIN operations o ON o.id = e.operation_id WHERE o.lane_id = $1 \
         AND o.id <= $2 AND o.id > $3 AND e.schema_version = 1 \
         AND e.payload->>'event' = 'compacted' \
         ORDER BY e.operation_id DESC, e.sequence DESC LIMIT 1",
    )
    .bind(lane.0)
    .bind(through.0)
    .bind(cutoff)
    .fetch_optional(connection)
    .await
    .map_err(Error::store)?;
    Ok(row.map_or_else(|| initial(cutoff), from_row))
}

const fn initial(operation: i64) -> Boundary {
    Boundary {
        operation,
        sequence: i64::MAX,
    }
}

const fn from_row((operation, sequence): (i64, i64)) -> Boundary {
    Boundary {
        operation,
        sequence,
    }
}
