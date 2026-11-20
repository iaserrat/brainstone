use crate::sqlite_backend::sqlite_error;
use crate::sqlite_backend::sqlite_error::store;
use sqlx::SqlSafeStr;
use sqlx::migrate::{Migration, MigrationType, Migrator};

pub(crate) fn migrator() -> Migrator {
    Migrator::with_migrations(vec![Migration::new(
        1,
        "durable runtime".into(),
        MigrationType::Simple,
        include_str!("../../sqlite_migrations/0001_runtime.sql").into_sql_str(),
        false,
    )])
}

/// Serialize schema inspection and migration bookkeeping across processes.
pub(crate) async fn migrate(pool: &sqlx::SqlitePool) -> Result<(), crate::Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(store)?;
    migrator()
        .run(&mut *transaction)
        .await
        .map_err(sqlite_error::migration)?;
    transaction.commit().await.map_err(store)
}
