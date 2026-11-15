use crate::Error;

#[allow(clippy::needless_pass_by_value, reason = "map_err callback")]
pub(crate) fn store(error: sqlx::Error) -> Error {
    if matches!(&error, sqlx::Error::ColumnDecode { .. } | sqlx::Error::Decode(_) | sqlx::Error::ColumnNotFound(_)) {
        return Error::corrupt(error);
    }
    let transient = match &error {
        sqlx::Error::Database(database) => database.code().and_then(|code| code.parse::<i32>().ok())
            .is_some_and(|code| matches!(code & 255, 5 | 6 | 10 | 13 | 14 | 15)),
        sqlx::Error::Io(_) | sqlx::Error::PoolClosed | sqlx::Error::PoolTimedOut | sqlx::Error::WorkerCrashed => true,
        _ => false,
    };
    if transient { Error::StoreUnavailable { message: error.to_string() } }
    else { Error::StoreFailed { message: error.to_string() } }
}

#[allow(clippy::needless_pass_by_value, reason = "map_err callback")]
pub(crate) fn migration(error: sqlx::migrate::MigrateError) -> Error {
    match error {
        sqlx::migrate::MigrateError::Execute(inner) | sqlx::migrate::MigrateError::ExecuteMigration(inner, _) => store(inner),
        other => Error::StoreFailed { message: other.to_string() },
    }
}

pub(crate) fn busy(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database) => database.code().and_then(|code| code.parse::<i32>().ok()).is_some_and(|code| matches!(code & 255, 5 | 6)),
        _ => false,
    }
}
