use sqlx::SqliteConnection;

use crate::{Admitted, Error, OperationId};

pub(crate) async fn steering(
    connection: &mut SqliteConnection,
    id: OperationId,
    duplicate: bool,
) -> Result<Admitted, Error> {
    let operation = crate::sqlite_read::operation_on(connection, id)
        .await?
        .ok_or_else(|| Error::CorruptRecord {
            message: "steering operation is missing".to_owned(),
        })?;
    Ok(Admitted::Steering {
        operation,
        duplicate,
    })
}
