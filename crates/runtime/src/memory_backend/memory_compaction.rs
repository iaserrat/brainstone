use std::collections::HashMap;

use crate::{Compaction, Error, LaneId, Operation, OperationId, OperationState};

#[derive(Debug, Default)]
pub(crate) struct Compactions {
    values: HashMap<LaneId, Compaction>,
}

impl Compactions {
    pub(crate) fn get(&self, lane_id: LaneId) -> Option<&Compaction> {
        self.values.get(&lane_id)
    }

    pub(crate) fn insert(
        &mut self,
        operations: &HashMap<OperationId, Operation>,
        compaction: &Compaction,
    ) -> Result<(), Error> {
        let Some(operation) = operations.get(&compaction.through) else {
            return Err(Error::InvalidCompaction);
        };
        if operation.lane_id != compaction.lane_id || !terminal(operation.state) {
            return Err(Error::InvalidCompaction);
        }
        if operations
            .values()
            .any(|item| item.lane_id == compaction.lane_id && item.state == OperationState::Running)
        {
            return Err(Error::InvalidCompaction);
        }
        if let Some(existing) = self.values.get(&compaction.lane_id) {
            validate_replacement(existing, compaction)?;
        }
        self.values.insert(compaction.lane_id, compaction.clone());
        Ok(())
    }
}

fn terminal(state: OperationState) -> bool {
    matches!(state, OperationState::Completed | OperationState::Failed)
}

fn validate_replacement(existing: &Compaction, new: &Compaction) -> Result<(), Error> {
    if new.through.0 < existing.through.0 {
        return Err(Error::InvalidCompaction);
    }
    if new.through == existing.through && new.summary != existing.summary {
        return Err(Error::InvalidCompaction);
    }
    Ok(())
}
