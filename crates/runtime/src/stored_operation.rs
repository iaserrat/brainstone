use crate::{Error, OperationState};
use agent::Outcome;
use serde_json::Value;
use sqlx::types::Json;

pub(crate) fn parse_outcome(
    state: OperationState,
    value: Option<&Json<Value>>,
) -> Result<Option<Outcome>, Error> {
    let outcome = value
        .map(|value| serde_json::from_value(value.0.clone()).map_err(Error::corrupt))
        .transpose()?;
    crate::operation::validation::validate_outcome(state, outcome.as_ref())?;
    Ok(outcome)
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

#[cfg(test)]
mod tests {
    use super::parse_outcome;
    use crate::OperationState;
    use serde_json::json;
    use sqlx::types::Json;

    #[test]
    fn database_outcomes_must_match_the_terminal_state() {
        let completed = Json(json!({"Completed": {"text": "done"}}));
        let failed = Json(json!({"Failed": {"code": "failed", "message": "failed"}}));
        for (state, outcome) in [
            (OperationState::Failed, completed),
            (OperationState::Completed, failed),
        ] {
            assert_eq!(
                parse_outcome(state, Some(&outcome)).unwrap_err().code(),
                "corrupt_record"
            );
        }
    }
}
