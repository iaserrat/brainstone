//! Tool media under the root conversation, pruned by age on every write.

use crate::sqlite_backend::sqlite_error::store;

use sqlx::{SqliteConnection, SqlitePool};

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
     FROM operations o JOIN lanes l ON l.id = o.lane_id WHERE o.id = ?1";

pub(crate) async fn save(pool: &SqlitePool, lease: &Lease, media: &[Media]) -> Result<(), Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(store)?;
    let root: i64 = sqlx::query_scalar(ROOT)
        .bind(lease.operation_id.0)
        .fetch_one(&mut *transaction)
        .await
        .map_err(store)?;
    for item in media {
        insert(&mut transaction, root, item).await?;
    }
    sqlx::query("DELETE FROM tool_media WHERE stored_at < CAST(unixepoch('subsec') * 1000 AS INTEGER) - (?1 * 86400000)")
        .bind(RETENTION_DAYS)
        .execute(&mut *transaction)
        .await
        .map_err(store)?;
    transaction.commit().await.map_err(store)
}

async fn insert(connection: &mut SqliteConnection, root: i64, item: &Media) -> Result<(), Error> {
    let (media_type, bytes, size, name) = columns(item)?;
    // Storing it again refreshes its age: a recapture keeps it shown.
    sqlx::query("INSERT INTO tool_media (root_session_id, reference, media_type, bytes, width, height, name) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT (root_session_id, reference) \
         DO UPDATE SET stored_at = EXCLUDED.stored_at")
        .bind(root).bind(item.reference()).bind(media_type).bind(bytes)
        .bind(size.map(|size| size.0)).bind(size.map(|size| size.1)).bind(name)
        .execute(connection).await.map_err(store)?;
    Ok(())
}

pub(crate) async fn load(
    pool: &SqlitePool,
    operation: OperationId,
    reference: &str,
) -> Result<Option<Media>, Error> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT m.media_type, m.bytes, m.width, m.height, m.name FROM tool_media m \
         JOIN operations o ON o.id = ?1 JOIN lanes l ON l.id = o.lane_id \
         WHERE m.root_session_id = COALESCE((SELECT root_session_id FROM subagent_runs \
         WHERE child_session_id = l.session_id), l.session_id) AND m.reference = ?2 \
         AND m.stored_at >= CAST(unixepoch('subsec') * 1000 AS INTEGER) - (?3 * 86400000)",
    )
    .bind(operation.0)
    .bind(reference)
    .bind(RETENTION_DAYS)
    .fetch_optional(pool)
    .await
    .map_err(store)?;
    row.map(parse).transpose()
}
