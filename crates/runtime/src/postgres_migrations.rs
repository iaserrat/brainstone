use sqlx::SqlSafeStr;
use sqlx::migrate::{Migration, MigrationType, Migrator};

#[cfg(test)]
#[path = "postgres_migrations_tests.rs"]
mod tests;

pub(crate) fn migrator() -> Migrator {
    Migrator::with_migrations(vec![Migration::new(
        1,
        "durable runtime".into(),
        MigrationType::Simple,
        include_str!("../migrations/0001_runtime.sql").into_sql_str(),
        false,
    )])
}
