use std::collections::HashMap;
use std::time::Instant;

use agent::Event;

use crate::memory_backend::memory_delivery::{Deliveries, Row};
use crate::{DeliveryCheckpoint, Error, LaneId, Operation, OperationId, OperationState};

impl Deliveries {
    pub(crate) fn checkpoint(
        &mut self,
        operations: &HashMap<OperationId, Operation>,
        checkpoint: &DeliveryCheckpoint,
    ) -> Result<(), Error> {
        let scope = checkpoint_row(&mut self.rows, operations, checkpoint)?;
        if let Some((lane, adapter)) = scope {
            invalidate_representative(&mut self.rows, operations, lane, &adapter);
        }
        Ok(())
    }
}

fn checkpoint_row(
    rows: &mut HashMap<OperationId, Row>,
    operations: &HashMap<OperationId, Operation>,
    checkpoint: &DeliveryCheckpoint,
) -> Result<Option<(LaneId, String)>, Error> {
    let row = rows
        .get_mut(&checkpoint.lease.operation_id)
        .ok_or(Error::LeaseLost)?;
    row.validate(&checkpoint.lease)?;
    if let Some(through) = checkpoint.through {
        Deliveries::advance(row, through, checkpoint.external_id.as_ref())?;
    } else if checkpoint.external_id.is_some() {
        // Postgres: external_id = COALESCE($3, external_id) for a presence
        // checkpoint, so a learned identity persists without advancing the
        // cursor.
        row.external_id.clone_from(&checkpoint.external_id);
    }
    let scope = terminal_scope(row, operations, checkpoint);
    row.presented = Some(Instant::now());
    row.lease = None;
    Ok(scope)
}

fn terminal_scope(
    row: &Row,
    operations: &HashMap<OperationId, Operation>,
    checkpoint: &DeliveryCheckpoint,
) -> Option<(LaneId, String)> {
    let through = usize::try_from(checkpoint.through?).ok()?;
    matches!(row.events.get(through), Some(Event::RunFinished { .. })).then(|| {
        let operation = &operations[&checkpoint.lease.operation_id];
        (operation.lane_id, row.target.adapter.clone())
    })
}

fn invalidate_representative(
    rows: &mut HashMap<OperationId, Row>,
    operations: &HashMap<OperationId, Operation>,
    lane: LaneId,
    adapter: &str,
) {
    let next = rows
        .iter()
        .filter(|(id, row)| is_live(operations.get(id), lane) && row.target.adapter == adapter)
        .map(|(id, _)| *id)
        .min_by_key(|id| id.0);
    if let Some(row) = next.and_then(|id| rows.get_mut(&id))
        && (row.lease.is_some() || row.presented.is_some())
    {
        row.invalidate();
    }
}

fn is_live(operation: Option<&Operation>, lane: LaneId) -> bool {
    operation.is_some_and(|operation| {
        operation.lane_id == lane
            && matches!(
                operation.state,
                OperationState::Accepted | OperationState::Running
            )
    })
}
