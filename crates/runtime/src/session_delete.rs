use crate::{Address, DurableStore, Error};

impl DurableStore {
    /// Permanently delete the addressed root session and its complete subagent tree.
    /// Every lane, operation, journal, approval, interaction, schedule and media record
    /// owned by that tree is removed atomically. The address's lane is ignored.
    /// Shared memories are independent and remain unchanged.
    /// Returns `false` when the root session does not exist.
    ///
    /// # Errors
    /// Returns `session_busy` for active execution or delivery leases,
    /// `invalid_input` for a subagent address, or a stable storage failure.
    /// This does not stop external work: the host must stop its workers first.
    pub async fn delete_session(&self, address: &Address) -> Result<bool, Error> {
        match self {
            Self::Sqlite(store) => sqlite::delete(store, address).await,
            Self::Postgres(store) => postgres::delete(store, address).await,
        }
    }
}

use crate::session_delete_store::backend;
backend!(
    sqlite,
    crate::SqliteStore,
    sqlx::SqliteConnection,
    crate::sqlite_error::store,
    "BEGIN IMMEDIATE",
    "CAST(unixepoch('subsec')*1000 AS INTEGER)",
    "",
    "SELECT value FROM json_each($1)"
);
backend!(
    postgres,
    crate::PostgresStore,
    sqlx::PgConnection,
    Error::store,
    "BEGIN",
    "clock_timestamp()",
    "FOR UPDATE",
    "SELECT value::bigint FROM jsonb_array_elements_text($1::jsonb)"
);
