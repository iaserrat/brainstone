use serde_json::Value;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::{DeliveryTarget, Error, Interaction, InteractionRequest};

type Data = Json<Value>;
type Row = (i64, Data, Data, String, Option<String>, Option<Data>);

pub(crate) async fn open(
    pool: &PgPool,
    request: &InteractionRequest,
) -> Result<Interaction, Error> {
    sqlx::query(
        "INSERT INTO interactions (id, operation_id, request) VALUES ($1, $2, $3) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(&request.id)
    .bind(request.operation_id.0)
    .bind(Json(&request.body))
    .execute(pool)
    .await
    .map_err(Error::store)?;
    let interaction = read(pool, &request.id).await?.ok_or_else(corrupt_missing)?;
    if interaction.request != *request {
        return Err(Error::IdempotencyConflict);
    }
    Ok(interaction)
}

pub(crate) async fn mark_delivered(pool: &PgPool, id: &str, ts: &str) -> Result<(), Error> {
    let updated = sqlx::query(
        "UPDATE interactions SET external_id = COALESCE(external_id, $2) \
         WHERE id = $1 AND (external_id IS NULL OR external_id = $2)",
    )
    .bind(id)
    .bind(ts)
    .execute(pool)
    .await
    .map_err(Error::store)?;
    require_row(updated.rows_affected())
}

pub(crate) async fn answer(pool: &PgPool, id: &str, response: &Value) -> Result<(), Error> {
    let updated = sqlx::query(
        "UPDATE interactions SET response = COALESCE(response, $2), \
         answered_at = COALESCE(answered_at, transaction_timestamp()) \
         WHERE id = $1 AND (response IS NULL OR response = $2)",
    )
    .bind(id)
    .bind(Json(response))
    .execute(pool)
    .await
    .map_err(Error::store)?;
    require_row(updated.rows_affected())
}

pub(crate) async fn read(pool: &PgPool, id: &str) -> Result<Option<Interaction>, Error> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT i.operation_id, i.request, o.delivery_target, \
         i.client_message_id::text, i.external_id, i.response \
         FROM interactions i JOIN operations o ON o.id = i.operation_id WHERE i.id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(Error::store)?;
    row.map(|row| from_row(id, row)).transpose()
}

fn from_row(id: &str, row: Row) -> Result<Interaction, Error> {
    let target = serde_json::from_value::<Option<DeliveryTarget>>(row.2.0)
        .map_err(Error::corrupt)?
        .ok_or(Error::InvalidInput {
            field: "interaction_delivery",
        })?;
    Ok(Interaction {
        request: InteractionRequest {
            operation_id: crate::OperationId(row.0),
            id: id.to_owned(),
            body: row.1.0,
        },
        target,
        client_message_id: row.3,
        external_id: row.4,
        response: row.5.map(|value| value.0),
    })
}

fn require_row(rows: u64) -> Result<(), Error> {
    (rows == 1).then_some(()).ok_or(Error::IdempotencyConflict)
}

fn corrupt_missing() -> Error {
    Error::CorruptRecord {
        message: "inserted interaction disappeared".to_owned(),
    }
}
