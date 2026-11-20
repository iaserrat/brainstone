use std::collections::HashMap;

use crate::{Operation, OperationId, OperationState};

pub(crate) fn finish(
    operations: &mut HashMap<OperationId, Operation>,
    id: OperationId,
    outcome: &agent::Outcome,
) -> Result<(), crate::Error> {
    let state = terminal_state(outcome)?;
    let operation = operations
        .get_mut(&id)
        .expect("a lease always names an operation");
    operation.state = state;
    operation.outcome = Some(outcome.clone());
    Ok(())
}

fn terminal_state(outcome: &agent::Outcome) -> Result<OperationState, crate::Error> {
    match outcome {
        agent::Outcome::Completed { .. } => Ok(OperationState::Completed),
        agent::Outcome::Failed { .. } => Ok(OperationState::Failed),
        _ => Err(crate::Error::corrupt("unknown outcome")),
    }
}
