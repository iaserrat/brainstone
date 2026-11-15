use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::memory_delivery::Deliveries;
use crate::{DeliveryBatch, DeliveryClaim, Error, LaneId, Operation, OperationId, OperationState};

impl Deliveries {
    pub(crate) fn claim(
        &mut self,
        operations: &HashMap<OperationId, Operation>,
        claim: &DeliveryClaim,
    ) -> Result<Option<DeliveryBatch>, Error> {
        let now = Instant::now();
        let expiry =
            now.checked_add(claim.ttl)
                .ok_or(Error::InvalidInput { field: "delivery_lease_ttl" })?;
        let representatives = representatives(&self.rows, operations, &claim.adapter);
        let id = self.rows.iter().filter_map(|(id, row)| {
            let represents = representatives.contains(id);
            (row.target.adapter == claim.adapter
                && !self.blocked(operations, *id)
                && row.available(now, operations.get(id), claim.presence, represents))
            .then_some(*id)
        });
        let Some(id) = id.min_by_key(|id| id.0) else {
            return Ok(None);
        };
        let row = self.rows.get_mut(&id).expect("selected delivery");
        row.token += 1;
        row.lease = Some((claim.worker.clone(), row.token, expiry));
        Ok(Some(row.batch(id, &claim.worker, claim.limit)))
    }

    fn blocked(&self, operations: &HashMap<OperationId, Operation>, id: OperationId) -> bool {
        let Some(row) = self.rows.get(&id) else {
            return true;
        };
        let Some(operation) = operations.get(&id) else {
            return true;
        };
        let predecessors = self.rows.iter().filter(|(prior, prior_row)| {
            prior.0 < id.0 && prior_row.target.adapter == row.target.adapter
        });
        predecessors.into_iter().any(|(prior, prior_row)| {
            operations.get(prior).is_some_and(|prior_operation| {
                prior_operation.lane_id == operation.lane_id
                    && pending(prior_row, prior_operation)
            })
        })
    }
}

fn pending(row: &super::Row, operation: &Operation) -> bool {
    row.cursor < row.events.len()
        || matches!(
            operation.state,
            OperationState::Accepted | OperationState::Running
        )
}

fn representatives(
    rows: &HashMap<OperationId, super::Row>,
    operations: &HashMap<OperationId, Operation>,
    adapter: &str,
) -> HashSet<OperationId> {
    let mut lanes = HashMap::<LaneId, OperationId>::new();
    for (id, _) in rows.iter().filter(|(_, row)| row.target.adapter == adapter) {
        let Some(lane) = live_lane(operations.get(id)) else {
            continue;
        };
        let current = lanes.entry(lane).or_insert(*id);
        if id.0 < current.0 {
            *current = *id;
        }
    }
    lanes.into_values().collect()
}

fn live_lane(operation: Option<&Operation>) -> Option<LaneId> {
    operation
        .filter(|operation| {
            matches!(
                operation.state,
                OperationState::Accepted | OperationState::Running
            )
        })
        .map(|operation| operation.lane_id)
}
