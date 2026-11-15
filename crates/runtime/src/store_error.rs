use sqlx::migrate::MigrateError;

use crate::Error;

/// SQLSTATE classes a later attempt can get past: connection exception,
/// insufficient resources, operator intervention (shutdown, cancel), and
/// system error.
const TRANSIENT_CLASSES: [&str; 4] = ["08", "53", "57", "58"];
/// Transactions Postgres aborted to resolve a conflict: serialization failure
/// and deadlock.
const TRANSIENT_CODES: [&str; 2] = ["40001", "40P01"];

impl Error {
    /// `store_unavailable` when the store could not be reached or aborted the
    /// work for a reason a retry can get past; `store_failed` when the database
    /// refused the operation itself, which then fails the same way every time.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "used as `map_err(Error::store)`"
    )]
    pub(crate) fn store(error: sqlx::Error) -> Self {
        Self::classified(transient(&error), error.to_string())
    }

    /// A migration failure takes the class of the database error beneath it.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "used as `map_err(Error::migration)`"
    )]
    pub(crate) fn migration(error: MigrateError) -> Self {
        let transient = match &error {
            MigrateError::Execute(inner) | MigrateError::ExecuteMigration(inner, _) => {
                transient(inner)
            }
            _ => false,
        };
        Self::classified(transient, error.to_string())
    }

    fn classified(transient: bool, message: String) -> Self {
        if transient {
            Self::StoreUnavailable { message }
        } else {
            Self::StoreFailed { message }
        }
    }
}

fn transient(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(error) => error.code().is_some_and(|code| {
            TRANSIENT_CODES.contains(&code.as_ref())
                || TRANSIENT_CLASSES
                    .iter()
                    .any(|class| code.starts_with(class))
        }),
        sqlx::Error::Io(_)
        | sqlx::Error::Tls(_)
        | sqlx::Error::Protocol(_)
        | sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::WorkerCrashed => true,
        _ => false,
    }
}
