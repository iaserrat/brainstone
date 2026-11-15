use serde::{Deserialize, Serialize};

use crate::{LaneId, OperationId};

/// A replacement of a lane's model-visible prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compaction {
    /// The lane compacted.
    pub lane_id: LaneId,
    /// The operation the summary replaces up to.
    pub through: OperationId,
    /// The summary that takes its place.
    pub summary: String,
}

impl Compaction {
    #[must_use]
    /// A compaction of `lane_id` through `through`.
    pub fn new(lane_id: LaneId, through: OperationId, summary: impl Into<String>) -> Self {
        Self {
            lane_id,
            through,
            summary: summary.into(),
        }
    }
}
