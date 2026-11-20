use crate::sqlite_backend::sqlite_error::store;
use crate::sqlite_backend::{sqlite_schedule_insert, sqlite_schedule_parse};
use sqlx::{SqliteConnection, SqlitePool};

use crate::sqlite_backend::sqlite_schedule_parse::{Row, schedule_columns};
use crate::{Error, ScheduleRequest, Scheduled};

pub(crate) async fn create(
    pool: &SqlitePool,
    request: &ScheduleRequest,
) -> Result<Scheduled, Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(store)?;
    let connection = &mut *transaction;
    let row = existing(connection, request).await?;
    let row = match row {
        Some(row) => validate(row, request)?,
        None => insert_or_existing(connection, request).await?,
    };
    transaction.commit().await.map_err(store)?;
    sqlite_schedule_parse::scheduled(&row)
}

async fn insert_or_existing(
    connection: &mut SqliteConnection,
    request: &ScheduleRequest,
) -> Result<Row, Error> {
    if let Some(row) = sqlite_schedule_insert::insert(connection, request).await? {
        return Ok(row);
    }
    let row = existing(connection, request)
        .await?
        .ok_or_else(|| Error::CorruptRecord {
            message: "conflicting schedule is missing".to_owned(),
        })?;
    validate(row, request)
}

async fn existing(
    connection: &mut SqliteConnection,
    request: &ScheduleRequest,
) -> Result<Option<Row>, Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        schedule_columns!(),
        " FROM scheduled_work s WHERE s.origin_operation_id = ?1 AND s.schedule_key = ?2"
    ))
    .bind(request.operation_id.0)
    .bind(&request.key)
    .fetch_optional(connection)
    .await
    .map_err(store)
}

/// An idempotent retry must describe the same work, timing, and destination.
fn validate(row: Row, request: &ScheduleRequest) -> Result<Row, Error> {
    let same = row.1 == request.prompt
        && sqlite_schedule_parse::time(&row)? == request.when
        && sqlite_schedule_parse::destination(&row)? == request.destination;
    if !same {
        return Err(Error::IdempotencyConflict);
    }
    Ok(row)
}
