use std::time::{Duration, Instant};

use crate::memory_backend::memory_delivery::Row;
use crate::{
    DeliveryBatch, DeliveryEvent, DeliveryLease, DeliveryTarget, Error, Operation, OperationId,
    OperationState,
};

impl Row {
    pub(crate) fn new(target: DeliveryTarget) -> Self {
        Self {
            target,
            events: Vec::new(),
            cursor: 0,
            external_id: None,
            lease: None,
            token: 0,
            presented: None,
            accepted_unix_ms: crate::schedule_repeat::now_unix_ms(),
        }
    }

    pub(crate) fn available(
        &self,
        now: Instant,
        operation: Option<&Operation>,
        presence: Option<Duration>,
        represents_lane: bool,
    ) -> bool {
        self.unlocked(now)
            && (self.has_events() || represents_lane && self.due(now, operation, presence))
    }

    fn unlocked(&self, now: Instant) -> bool {
        self.lease
            .as_ref()
            .is_none_or(|(_, _, expiry)| *expiry <= now)
    }

    fn has_events(&self) -> bool {
        self.cursor < self.events.len()
    }

    fn due(&self, now: Instant, operation: Option<&Operation>, presence: Option<Duration>) -> bool {
        let Some(presence) = presence else {
            return false;
        };
        if !operation.is_some_and(|item| {
            matches!(
                item.state,
                OperationState::Accepted | OperationState::Running
            )
        }) {
            return false;
        }
        self.presented
            .is_none_or(|presented| now.duration_since(presented) >= presence)
    }

    pub(crate) fn batch(&self, id: OperationId, worker: &str, limit: usize) -> DeliveryBatch {
        let events = self
            .events
            .iter()
            .enumerate()
            .skip(self.cursor)
            .take(limit)
            .map(|(sequence, event)| DeliveryEvent {
                sequence: i64::try_from(sequence).expect("test event sequence fits i64"),
                event: event.clone(),
            })
            .collect();
        DeliveryBatch {
            lease: DeliveryLease {
                operation_id: id,
                worker: worker.to_owned(),
                token: self.token,
            },
            target: self.target.clone(),
            external_id: self.external_id.clone(),
            events,
            notice: None,
            accepted_unix_ms: self.accepted_unix_ms,
        }
    }

    pub(crate) fn validate(&self, lease: &DeliveryLease) -> Result<(), Error> {
        match &self.lease {
            Some((worker, token, expiry))
                if worker == &lease.worker && token == &lease.token && *expiry > Instant::now() =>
            {
                Ok(())
            }
            _ => Err(Error::LeaseLost),
        }
    }
}
