use std::time::{Duration, Instant};

use crate::memory_delivery::Deliveries;
use crate::{DeliveryLease, Error};

impl Deliveries {
    pub(crate) fn renew(&mut self, lease: &DeliveryLease, ttl: Duration) -> Result<(), Error> {
        let row = self
            .rows
            .get_mut(&lease.operation_id)
            .ok_or(Error::LeaseLost)?;
        row.validate(lease)?;
        let expiry = Instant::now()
            .checked_add(ttl)
            .ok_or(Error::InvalidInput { field: "lease_ttl" })?;
        let (_, _, current_expiry) = row.lease.as_mut().expect("validated delivery lease");
        *current_expiry = expiry;
        Ok(())
    }
}
