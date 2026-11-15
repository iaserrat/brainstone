use sqlx::PgConnection;

use crate::{Admitted, Error, OperationId};

pub(crate) async fn steering(
    connection: &mut PgConnection,
    id: OperationId,
    duplicate: bool,
) -> Result<Admitted, Error> {
    let operation = crate::postgres_read::operation_on(connection, id)
        .await?
        .ok_or_else(|| Error::CorruptRecord {
            message: "steering operation is missing".to_owned(),
        })?;
    Ok(Admitted::Steering {
        operation,
        duplicate,
    })
}
