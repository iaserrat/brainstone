use crate::{Error, OperationState};
use agent::Outcome;
use serde_json::Value;
use sqlx::types::Json;

pub(crate) fn parse_outcome(
    state: OperationState,
    value: Option<&Json<Value>>,
) -> Result<Option<Outcome>, Error> {
    let terminal = matches!(state, OperationState::Completed | OperationState::Failed);
    match (terminal, value) {
        (false, None) => Ok(None),
        (true, Some(value)) => serde_json::from_value(value.0.clone())
            .map(Some)
            .map_err(Error::corrupt),
        _ => Err(Error::CorruptRecord {
            message: "operation state and outcome disagree".to_owned(),
        }),
    }
}

pub(crate) fn parse_state(value: &str) -> Result<OperationState, Error> {
    match value {
        "accepted" => Ok(OperationState::Accepted),
        "running" => Ok(OperationState::Running),
        "completed" => Ok(OperationState::Completed),
        "failed" => Ok(OperationState::Failed),
        other => Err(Error::CorruptRecord {
            message: format!("unknown operation state: {other}"),
        }),
    }
}
