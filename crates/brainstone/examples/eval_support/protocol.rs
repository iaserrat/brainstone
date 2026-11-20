//! Synthetic evaluation inputs contain no credentials.
use std::io::Write;

use brainstone::{Operation, Outcome};
use serde::Deserialize;
use serde_json::{Value, json};

pub(crate) const CRASH: i32 = 73;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Action {
    Invoke,
    Resume,
    Inspect,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Input {
    pub(crate) action: Action,
    pub(crate) database: String,
    pub(crate) effects_database: String,
    pub(crate) conversation: String,
    pub(crate) request_id: String,
    pub(crate) system: String,
    pub(crate) text: String,
    pub(crate) endpoint: String,
    pub(crate) model: String,
    pub(crate) fault: Option<String>,
    pub(crate) operation_id: Option<i64>,
    pub(crate) output_schema: Option<Value>,
}

pub(crate) fn emit(value: &Value) {
    println!("{value}");
    std::io::stdout()
        .flush()
        .expect("evaluation stdout is open");
}

pub(crate) fn terminal(operation: Operation) {
    let id = operation.id.get();
    let value = match operation.outcome {
        Some(Outcome::Completed { text }) => {
            json!({"event":"terminal", "status":"completed", "operation_id":id, "value":text})
        }
        Some(Outcome::Failed { code, message }) => {
            json!({"event":"terminal", "status":"failed", "operation_id":id, "code":code, "message":message})
        }
        _ => json!({"event":"pending", "operation_id":id}),
    };
    emit(&value);
}

#[cfg(test)]
mod tests {
    use super::Action;

    #[test]
    fn unknown_actions_cannot_submit_work() {
        assert!(serde_json::from_str::<Action>("\"invented\"").is_err());
    }
}
