use agent::{Event, Message, Outcome};
use sqlx::SqliteConnection;

use crate::sqlite_steering_pending::Pending;
use crate::{Error, Lease};

pub(crate) async fn lock(connection: &mut SqliteConnection, lease: &Lease) -> Result<i64, Error> {
    sqlx::query_scalar(
        "SELECT o.next_event_sequence FROM operations o JOIN lanes l ON l.id = o.lane_id \
         JOIN sessions s ON s.id = l.session_id WHERE o.id = ?1 AND s.id = ?2 \
         AND s.lease_owner = ?3 AND s.lease_token = ?4 AND o.lease_token = ?4 \
         AND s.lease_expires_at > CAST(unixepoch('subsec') * 1000 AS INTEGER) AND o.state = 'running' \
        ",
    )
    .bind(lease.operation_id.0)
    .bind(lease.session_id.0)
    .bind(&lease.worker)
    .bind(lease.token)
    .fetch_optional(connection)
    .await
    .map_err(crate::sqlite_error::store)?
    .ok_or(Error::LeaseLost)
}

pub(crate) async fn append_users(
    connection: &mut SqliteConnection,
    lease: &Lease,
    sequence: i64,
    pending: &[Pending],
) -> Result<Vec<Message>, Error> {
    let mut messages = Vec::with_capacity(pending.len());
    for (offset, entry) in pending.iter().enumerate() {
        let message = entry.message();
        let event = Event::Appended {
            message: message.clone(),
        };
        crate::sqlite_steering_events::insert(
            connection,
            lease,
            sequence + i64::try_from(offset).map_err(Error::corrupt)?,
            &event,
        )
        .await?;
        messages.push(message);
    }
    crate::sqlite_steering_events::advance(
        connection,
        lease,
        sequence + i64::try_from(pending.len()).map_err(Error::corrupt)?,
    )
    .await?;
    crate::sqlite_steering_events::consume(connection, pending).await?;
    Ok(messages)
}

pub(crate) async fn finish(
    connection: &mut SqliteConnection,
    lease: &Lease,
    sequence: i64,
    outcome: &Outcome,
) -> Result<(), Error> {
    let event = Event::RunFinished {
        outcome: outcome.clone(),
    };
    crate::sqlite_steering_events::insert(connection, lease, sequence, &event).await?;
    crate::sqlite_steering_events::advance(connection, lease, sequence + 1).await?;
    crate::sqlite_finish::finish(connection, lease, outcome).await?;
    crate::sqlite_subagent_notify::notify(connection, lease.operation_id, outcome).await
}
