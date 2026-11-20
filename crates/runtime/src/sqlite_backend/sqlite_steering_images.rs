use crate::sqlite_backend::sqlite_error::store;
use crate::sqlite_backend::sqlite_images;
use agent::Image;
use sqlx::SqliteConnection;

use crate::Error;

pub(crate) async fn insert(
    connection: &mut SqliteConnection,
    steering: i64,
    images: &[Image],
) -> Result<(), Error> {
    for (ordinal, image) in images.iter().enumerate() {
        sqlx::query("INSERT INTO operation_steering_images (steering_id, ordinal, media_type, bytes, width, height) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
            .bind(steering).bind(i16::try_from(ordinal).map_err(Error::corrupt)?)
            .bind(image.media_type().as_str()).bind(image.bytes())
            .bind(i32::try_from(image.width()).map_err(Error::corrupt)?)
            .bind(i32::try_from(image.height()).map_err(Error::corrupt)?)
            .execute(&mut *connection).await.map_err(store)?;
    }
    Ok(())
}

pub(crate) async fn load(
    connection: &mut SqliteConnection,
    steering: i64,
    expected: i16,
) -> Result<Vec<Image>, Error> {
    let rows = sqlx::query_as("SELECT steering_id, ordinal, media_type, bytes, width, height FROM operation_steering_images WHERE steering_id = ?1 ORDER BY ordinal")
        .bind(steering).fetch_all(connection).await.map_err(store)?;
    sqlite_images::parse_group(rows, steering, expected)
}
