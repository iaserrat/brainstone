use crate::DurableStore;

pub(crate) fn sql(store: &DurableStore, source: &str) -> String {
    match store {
        DurableStore::Postgres(_) => source.replace("@lease_now", "clock_timestamp()").replace(
            "@now",
            "CAST(EXTRACT(EPOCH FROM clock_timestamp())*1000 AS BIGINT)",
        ),
        DurableStore::Sqlite(_) => source
            .replace("@lease_now", "@now")
            .replace('$', "?")
            .replace("@now", "CAST(unixepoch('subsec')*1000 AS INTEGER)"),
    }
}

// Both databases return the same owned tuple; only SQL dialect and pool differ.
macro_rules! rows {
    ($store:expr, $source:expr, $row:ty, $fetch:ident $(, $arg:expr)* $(,)?) => {{
        let statement = $crate::operator_db::sql($store, $source);
        match $store {
            $crate::DurableStore::Postgres(store) => sqlx::query_as::<_, $row>(sqlx::AssertSqlSafe(statement.as_str()))$(.bind($arg))*. $fetch(&store.pool).await,
            $crate::DurableStore::Sqlite(store) => sqlx::query_as::<_, $row>(sqlx::AssertSqlSafe(statement.as_str()))$(.bind($arg))*. $fetch(&store.pool).await,
        }.map_err(|error| $crate::operator_db::error($store, error))
    }};
}
macro_rules! execute {
    ($store:expr, $source:expr $(, $arg:expr)* $(,)?) => {{
        let statement = $crate::operator_db::sql($store, $source);
        match $store {
            $crate::DurableStore::Postgres(store) => sqlx::query(sqlx::AssertSqlSafe(statement.as_str()))$(.bind($arg))*.execute(&store.pool).await.map(|r|r.rows_affected()),
            $crate::DurableStore::Sqlite(store) => sqlx::query(sqlx::AssertSqlSafe(statement.as_str()))$(.bind($arg))*.execute(&store.pool).await.map(|r|r.rows_affected()),
        }.map_err(|error| $crate::operator_db::error($store, error))
    }};
}
pub(crate) use {execute, rows};
pub(crate) fn error(store: &DurableStore, error: sqlx::Error) -> crate::Error {
    match store {
        DurableStore::Postgres(_) => crate::Error::store(error),
        DurableStore::Sqlite(_) => crate::sqlite_error::store(error),
    }
}
