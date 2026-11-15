use sqlx::PgPool;

use crate::Error;

pub(crate) async fn verify(pool: &PgPool) -> Result<(), Error> {
    let synchronous_commit = setting(pool, "SHOW synchronous_commit").await?;
    require_on("synchronous_commit", &synchronous_commit)?;
    let fsync = setting(pool, "SHOW fsync").await?;
    require_on("fsync", &fsync)?;
    let full_page_writes = setting(pool, "SHOW full_page_writes").await?;
    require_on("full_page_writes", &full_page_writes)
}

async fn setting(pool: &PgPool, query: &'static str) -> Result<String, Error> {
    sqlx::query_scalar(query)
        .fetch_one(pool)
        .await
        .map_err(Error::store)
}

pub(crate) fn require_on(setting: &'static str, value: &str) -> Result<(), Error> {
    if value == "on" {
        return Ok(());
    }
    Err(Error::DurabilityNotGuaranteed {
        setting,
        value: value.to_owned(),
    })
}
