use agent::Image;
use sqlx::PgConnection;

use crate::{Error, OperationId};

pub(crate) use crate::stored_images::{Row, parse_group};

pub(crate) async fn insert(
    connection: &mut PgConnection,
    operation: OperationId,
    images: &[Image],
) -> Result<(), Error> {
    for (ordinal, image) in images.iter().enumerate() {
        sqlx::query("INSERT INTO operation_images (operation_id, ordinal, media_type, bytes, width, height) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(operation.0).bind(i16::try_from(ordinal).map_err(Error::corrupt)?)
            .bind(image.media_type().as_str()).bind(image.bytes())
            .bind(i32::try_from(image.width()).map_err(Error::corrupt)?)
            .bind(i32::try_from(image.height()).map_err(Error::corrupt)?)
            .execute(&mut *connection).await.map_err(Error::store)?;
    }
    Ok(())
}

pub(crate) async fn load(
    connection: &mut PgConnection,
    operation: OperationId,
    expected: i16,
) -> Result<Vec<Image>, Error> {
    let rows = sqlx::query_as("SELECT operation_id, ordinal, media_type, bytes, width, height FROM operation_images WHERE operation_id = $1 ORDER BY ordinal")
        .bind(operation.0).fetch_all(connection).await.map_err(Error::store)?;
    parse_group(rows, operation.0, expected)
}
