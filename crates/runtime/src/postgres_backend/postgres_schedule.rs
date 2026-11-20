use crate::postgres_backend::{postgres_schedule_insert, postgres_schedule_parse};
use sqlx::{PgConnection, PgPool};

use crate::postgres_backend::postgres_schedule_parse::{Row, schedule_columns};
use crate::{Error, ScheduleRequest, Scheduled};

pub(crate) async fn create(pool: &PgPool, request: &ScheduleRequest) -> Result<Scheduled, Error> {
    let mut transaction = pool.begin().await.map_err(Error::store)?;
    let connection = &mut *transaction;
    let row = existing(connection, request).await?;
    let row = match row {
        Some(row) => validate(row, request)?,
        None => insert_or_existing(connection, request).await?,
    };
    transaction.commit().await.map_err(Error::store)?;
    postgres_schedule_parse::scheduled(&row)
}

async fn insert_or_existing(
    connection: &mut PgConnection,
    request: &ScheduleRequest,
) -> Result<Row, Error> {
    if let Some(row) = postgres_schedule_insert::insert(connection, request).await? {
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
    connection: &mut PgConnection,
    request: &ScheduleRequest,
) -> Result<Option<Row>, Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        schedule_columns!(),
        " FROM scheduled_work s WHERE s.origin_operation_id = $1 AND s.schedule_key = $2"
    ))
    .bind(request.operation_id.0)
    .bind(&request.key)
    .fetch_optional(connection)
    .await
    .map_err(Error::store)
}

/// An idempotent retry must describe the same work, timing, and destination.
fn validate(row: Row, request: &ScheduleRequest) -> Result<Row, Error> {
    let same = row.1 == request.prompt
        && postgres_schedule_parse::time(&row)? == request.when
        && postgres_schedule_parse::destination(&row)? == request.destination;
    if !same {
        return Err(Error::IdempotencyConflict);
    }
    Ok(row)
}
