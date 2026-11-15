use sqlx::{PgConnection, PgPool};

use crate::{DeliveryCheckpoint, Error};

#[cfg(test)]
#[path = "postgres_delivery_checkpoint_tests.rs"]
mod tests;

#[path = "postgres_delivery_reconcile.rs"]
mod reconcile;

type State = (Option<String>, i64, bool, i64, i64, bool);

pub(crate) async fn checkpoint(
    pool: &PgPool,
    checkpoint: &DeliveryCheckpoint,
) -> Result<(), Error> {
    let mut transaction = pool.begin().await.map_err(Error::store)?;
    let terminal = validate(&mut transaction, checkpoint).await?;
    update(&mut transaction, checkpoint, terminal).await?;
    if terminal {
        reconcile::invalidate(&mut transaction, checkpoint).await?;
    }
    transaction.commit().await.map_err(Error::store)
}

async fn validate(
    connection: &mut PgConnection,
    checkpoint: &DeliveryCheckpoint,
) -> Result<bool, Error> {
    let state: State = sqlx::query_as(
        "SELECT d.lease_owner, d.lease_token, \
         COALESCE(d.lease_expires_at > clock_timestamp(), FALSE), \
         d.next_event_sequence, o.next_event_sequence, \
         o.state IN ('completed', 'failed') FROM operation_deliveries d \
         JOIN operations o ON o.id = d.operation_id WHERE d.operation_id = $1 FOR UPDATE OF d",
    )
    .bind(checkpoint.lease.operation_id.0)
    .fetch_optional(connection)
    .await
    .map_err(Error::store)?
    .ok_or(Error::LeaseLost)?;
    validate_state(&state, checkpoint)
}

fn validate_state(state: &State, checkpoint: &DeliveryCheckpoint) -> Result<bool, Error> {
    let owns = state.0.as_deref() == Some(&checkpoint.lease.worker)
        && state.1 == checkpoint.lease.token
        && state.2;
    if !owns {
        return Err(Error::LeaseLost);
    }
    match checkpoint.through {
        Some(through) if through < state.3 || through >= state.4 => {
            Err(Error::InvalidDeliveryCheckpoint)
        }
        _ => Ok(terminal_checkpoint(state.5, state.4, checkpoint.through)),
    }
}

pub(super) fn terminal_checkpoint(terminal: bool, next: i64, through: Option<i64>) -> bool {
    terminal && through.and_then(|value| value.checked_add(1)) == Some(next)
}

async fn update(
    connection: &mut PgConnection,
    checkpoint: &DeliveryCheckpoint,
    terminal: bool,
) -> Result<(), Error> {
    sqlx::query(crate::postgres_delivery_sql::CHECKPOINT)
        .bind(checkpoint.lease.operation_id.0)
        .bind(checkpoint.through)
        .bind(&checkpoint.external_id)
        .bind(terminal)
        .execute(connection)
        .await
        .map_err(Error::store)?;
    Ok(())
}
