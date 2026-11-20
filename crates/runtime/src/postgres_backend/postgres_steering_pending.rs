//! Unconsumed steering rows, loaded with their attachments.

use crate::postgres_backend::postgres_steering_images;

use agent::Image;
use sqlx::PgConnection;

use crate::{Error, Lease};

/// One unconsumed steering prompt and everything its user turn carries.
pub(crate) struct Pending {
    pub(crate) id: i64,
    pub(crate) text: String,
    pub(crate) images: Vec<Image>,
}

impl Pending {
    /// The user turn this steering row appends to the thread.
    pub(crate) fn message(&self) -> agent::Message {
        agent::Message::user(self.text.clone(), self.images.clone())
    }
}

pub(crate) async fn pending(
    connection: &mut PgConnection,
    lease: &Lease,
) -> Result<Vec<Pending>, Error> {
    let rows: Vec<(i64, String, i16)> = sqlx::query_as(
        "SELECT id, prompt, image_count FROM operation_steering \
         WHERE operation_id = $1 AND consumed = FALSE ORDER BY id LIMIT 1 FOR UPDATE",
    )
    .bind(lease.operation_id.0)
    .fetch_all(&mut *connection)
    .await
    .map_err(Error::store)?;
    let mut pending = Vec::with_capacity(rows.len());
    for (id, text, images) in rows {
        let images = postgres_steering_images::load(connection, id, images).await?;
        pending.push(Pending { id, text, images });
    }
    Ok(pending)
}
