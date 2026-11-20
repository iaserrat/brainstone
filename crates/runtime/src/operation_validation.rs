use agent::Outcome;
use serde::Deserialize;

use super::{LaneId, Operation, OperationId, OperationState, SessionId};
use crate::Error;

#[derive(Deserialize)]
pub(super) struct RawOperation {
    id: OperationId,
    session_id: SessionId,
    lane_id: LaneId,
    state: OperationState,
    outcome: Option<Outcome>,
}

impl TryFrom<RawOperation> for Operation {
    type Error = Error;

    fn try_from(raw: RawOperation) -> Result<Self, Self::Error> {
        let operation = Self {
            id: raw.id,
            session_id: raw.session_id,
            lane_id: raw.lane_id,
            state: raw.state,
            outcome: raw.outcome,
        };
        operation.validate()?;
        Ok(operation)
    }
}

impl Operation {
    /// Check the state/outcome contract after custom-store construction or mutation.
    ///
    /// # Errors
    /// Returns `corrupt_record` when the state disagrees with the outcome.
    pub fn validate(&self) -> Result<(), Error> {
        validate_outcome(self.state, self.outcome.as_ref())
    }
}

pub(crate) fn validate_outcome(
    state: OperationState,
    outcome: Option<&Outcome>,
) -> Result<(), Error> {
    match (state, outcome) {
        (OperationState::Accepted | OperationState::Running, None)
        | (OperationState::Completed, Some(Outcome::Completed { .. }))
        | (OperationState::Failed, Some(Outcome::Failed { .. })) => Ok(()),
        _ => Err(Error::corrupt("operation state and outcome disagree")),
    }
}
