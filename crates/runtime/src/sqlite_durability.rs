use sqlx::SqlitePool;
use crate::Error;

pub(crate) async fn verify(pool: &SqlitePool) -> Result<(), Error> {
    let journal: String = sqlx::query_scalar("PRAGMA journal_mode").fetch_one(pool).await.map_err(crate::sqlite_error::store)?;
    require("sqlite_journal_mode", &journal, "wal")?;
    let sync: i64 = sqlx::query_scalar("PRAGMA synchronous").fetch_one(pool).await.map_err(crate::sqlite_error::store)?;
    require("sqlite_synchronous", &sync.to_string(), "2")?;
    let keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys").fetch_one(pool).await.map_err(crate::sqlite_error::store)?;
    require("sqlite_foreign_keys", &keys.to_string(), "1")
}

fn require(setting: &'static str, value: &str, expected: &str) -> Result<(), Error> {
    if value == expected { return Ok(()); }
    Err(Error::DurabilityNotGuaranteed { setting, value: value.to_owned() })
}
