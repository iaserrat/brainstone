use crate::{Error, PostgresStore, SqliteStore};

/// A complete application backend, selected once at startup.
/// Both variants implement every persistence capability used by the host and surfaces.
#[derive(Debug, Clone)]
pub enum DurableStore {
/// A PostgreSQL backend.
    Postgres(PostgresStore),
/// A SQLite backend.
    Sqlite(SqliteStore),
}

macro_rules! dispatch {
    ($store:expr, $method:ident($($arg:expr),* $(,)?)) => {
        match $store {
            DurableStore::Postgres(store) => store.$method($($arg),*).await,
            DurableStore::Sqlite(store) => store.$method($($arg),*).await,
        }
    };
}
pub(crate) use dispatch;

impl DurableStore {
    /// Connect and migrate the backend named by a `PostgreSQL` or `SQLite` URL.
    ///
    /// # Errors
    /// Returns the backend's stable setup, migration, or durability failure.
    pub async fn connect(url: &str) -> Result<Self, Error> {
        if url.starts_with("sqlite:") {
            return SqliteStore::connect(url).await.map(Self::Sqlite);
        }
        PostgresStore::connect(url).await.map(Self::Postgres)
    }

    /// # Errors
    /// Returns the backend's stable connection failure.
    pub async fn ping(&self) -> Result<(), Error> {
        dispatch!(self, ping())
    }

/// Closes whichever backend is selected.
    pub async fn close(&self) {
        dispatch!(self, close());
    }
}
