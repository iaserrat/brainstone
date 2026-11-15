//! Tool media under the root conversation, pruned by age on every write.

use sqlx::{PgConnection, PgPool};

use crate::{Error, Lease, Media, OperationId};

use crate::stored_media_row as row;
use row::{Row, columns, parse};

/// How long a reply can still show media after a tool returned it. Files are
/// tens of megabytes, so the table is bounded by age.
const RETENTION_DAYS: i32 = 7;

/// The root session: the operation's own, or the one its subagent run
/// descends from. `load` joins the same rule.
const ROOT: &str = "SELECT COALESCE((SELECT root_session_id FROM subagent_runs \
     WHERE child_session_id = l.session_id), l.session_id) \
     FROM operations o JOIN lanes l ON l.id = o.lane_id WHERE o.id = $1";

pub(crate) async fn save(pool: &PgPool, lease: &Lease, media: &[Media]) -> Result<(), Error> {
    let mut transaction = pool.begin().await.map_err(Error::store)?;
    let root: i64 = sqlx::query_scalar(ROOT)
        .bind(lease.operation_id.0)
        .fetch_one(&mut *transaction)
        .await
        .map_err(Error::store)?;
    for item in media {
        insert(&mut transaction, root, item).await?;
    }
    sqlx::query("DELETE FROM tool_media WHERE stored_at < transaction_timestamp() - make_interval(days => $1)")
        .bind(RETENTION_DAYS)
        .execute(&mut *transaction)
        .await
        .map_err(Error::store)?;
    transaction.commit().await.map_err(Error::store)
}

async fn insert(connection: &mut PgConnection, root: i64, item: &Media) -> Result<(), Error> {
    let (media_type, bytes, size, name) = columns(item)?;
    // Storing it again refreshes its age: a recapture keeps it shown.
    sqlx::query("INSERT INTO tool_media (root_session_id, reference, media_type, bytes, width, height, name) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (root_session_id, reference) \
         DO UPDATE SET stored_at = EXCLUDED.stored_at")
        .bind(root).bind(item.reference()).bind(media_type).bind(bytes)
        .bind(size.map(|size| size.0)).bind(size.map(|size| size.1)).bind(name)
        .execute(connection).await.map_err(Error::store)?;
    Ok(())
}

pub(crate) async fn load(
    pool: &PgPool,
    operation: OperationId,
    reference: &str,
) -> Result<Option<Media>, Error> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT m.media_type, m.bytes, m.width, m.height, m.name FROM tool_media m \
         JOIN operations o ON o.id = $1 JOIN lanes l ON l.id = o.lane_id \
         WHERE m.root_session_id = COALESCE((SELECT root_session_id FROM subagent_runs \
         WHERE child_session_id = l.session_id), l.session_id) AND m.reference = $2 \
         AND m.stored_at >= transaction_timestamp() - make_interval(days => $3)",
    )
    .bind(operation.0).bind(reference).bind(RETENTION_DAYS)
    .fetch_optional(pool).await.map_err(Error::store)?;
    row.map(parse).transpose()
}
