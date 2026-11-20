use std::collections::HashMap;
use std::time::Instant;

use agent::Event;

use crate::{DeliveryTarget, Error, OperationId};

#[path = "memory_delivery_claim.rs"]
mod claim;
#[path = "memory_delivery_lease.rs"]
mod lease;

#[derive(Debug, Default)]
pub(crate) struct Deliveries {
    pub(crate) rows: HashMap<OperationId, Row>,
}

#[derive(Debug)]
pub(crate) struct Row {
    pub(crate) target: DeliveryTarget,
    pub(crate) events: Vec<Event>,
    pub(crate) cursor: usize,
    pub(crate) external_id: Option<String>,
    pub(crate) lease: Option<(String, i64, Instant)>,
    pub(crate) token: i64,
    pub(crate) presented: Option<Instant>,
    pub(crate) accepted_unix_ms: i64,
}

impl Row {
    pub(crate) fn invalidate(&mut self) {
        self.presented = None;
        self.lease = None;
        self.token += 1;
    }
}

impl Deliveries {
    pub(crate) fn register(&mut self, id: OperationId, target: Option<DeliveryTarget>) {
        let Some(target) = target else { return };
        self.rows.insert(id, Row::new(target));
    }

    pub(crate) fn record(&mut self, id: OperationId, event: &Event) {
        if let Some(row) = self.rows.get_mut(&id) {
            if row.cursor == row.events.len() && row.lease.is_some() {
                row.invalidate();
            }
            row.events.push(event.clone());
        }
    }

    pub(crate) fn advance(
        row: &mut Row,
        through: i64,
        external_id: Option<&String>,
    ) -> Result<(), Error> {
        let next = through
            .checked_add(1)
            .and_then(|next| usize::try_from(next).ok())
            .ok_or(Error::InvalidDeliveryCheckpoint)?;
        if next <= row.cursor || next > row.events.len() {
            return Err(Error::InvalidDeliveryCheckpoint);
        }
        row.cursor = next;
        row.external_id = external_id.cloned();
        Ok(())
    }

    pub(crate) fn invalidate_presentation(&mut self, id: OperationId) {
        if let Some(row) = self.rows.get_mut(&id) {
            row.invalidate();
        }
    }

    pub(crate) fn save_stream(
        &mut self,
        lease: &crate::DeliveryLease,
        external_id: Option<&str>,
    ) -> Result<(), Error> {
        let row = self
            .rows
            .get_mut(&lease.operation_id)
            .ok_or(Error::LeaseLost)?;
        row.validate(lease)?;
        row.external_id = external_id.map(ToOwned::to_owned);
        Ok(())
    }
}
