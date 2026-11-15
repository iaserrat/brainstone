use serde_json::Value;
use sqlx::types::Json;
use sqlx::{PgConnection, Row};

use crate::{DeliveryClaim, DeliveryTarget, Error, OperationId};

pub(crate) struct Claimed {
    pub(crate) id: OperationId,
    pub(crate) cursor: i64,
    pub(crate) token: i64,
    pub(crate) worker: String,
    pub(crate) target: DeliveryTarget,
    pub(crate) external_id: Option<String>,
    /// The failure streak of the standing schedule this operation ran for.
    pub(crate) consecutive_failures: Option<i16>,
    pub(crate) accepted_unix_ms: i64,
}

pub(crate) async fn claim(
    connection: &mut PgConnection,
    claim: &DeliveryClaim,
) -> Result<Option<Claimed>, Error> {
    let row = select(connection, claim).await?;
    row.as_ref()
        .map(|row| from_row(row, &claim.worker))
        .transpose()
}

async fn select(
    connection: &mut PgConnection,
    claim: &DeliveryClaim,
) -> Result<Option<sqlx::postgres::PgRow>, Error> {
    let ttl = i64::try_from(claim.ttl.as_millis()).map_err(|_| Error::InvalidInput {
        field: "delivery_lease_ttl",
    })?;
    sqlx::query(crate::postgres_delivery_sql::CLAIM)
        .bind(&claim.worker)
        .bind(ttl)
        .bind(&claim.adapter)
        .bind(presence_ms(claim)?)
        .fetch_optional(connection)
        .await
        .map_err(Error::store)
}

fn presence_ms(claim: &DeliveryClaim) -> Result<Option<i64>, Error> {
    claim
        .presence
        .map(|presence| i64::try_from(presence.as_millis()).map_err(Error::corrupt))
        .transpose()
}

fn from_row(row: &sqlx::postgres::PgRow, worker: &str) -> Result<Claimed, Error> {
    let target: Json<Value> = row.try_get(4).map_err(Error::store)?;
    Ok(Claimed {
        id: OperationId(row.try_get(0).map_err(Error::store)?),
        cursor: row.try_get(1).map_err(Error::store)?,
        token: row.try_get(2).map_err(Error::store)?,
        external_id: row.try_get(3).map_err(Error::store)?,
        consecutive_failures: row.try_get(5).map_err(Error::store)?,
        accepted_unix_ms: row.try_get(6).map_err(Error::store)?,
        worker: worker.to_owned(),
        target: serde_json::from_value(target.0).map_err(Error::corrupt)?,
    })
}
