use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Error;

/// Where deliveries go: a surface adapter and its own address data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryTarget {
    /// The surface adapter's name.
    pub adapter: String,
    /// The adapter's own addressing data, opaque here.
    pub data: Value,
}

impl DeliveryTarget {
    /// # Errors
    ///
    /// Returns `invalid_input` when the adapter name is empty.
    pub fn new(adapter: &str, data: Value) -> Result<Self, Error> {
        if adapter.is_empty() {
            return Err(Error::InvalidInput {
                field: "delivery_adapter",
            });
        }
        Ok(Self {
            adapter: adapter.to_owned(),
            data,
        })
    }
}
