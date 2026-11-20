//! Shared process protocol; only the gateway receives real credentials.
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Write;

pub const CRASH_EXIT: i32 = 73;
pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Invoke,
    Resume,
    Inspect,
}

#[derive(Debug, Deserialize)]
pub struct Input {
    pub action: Action,
    pub database: String,
    pub effects_database: String,
    pub conversation: String,
    pub request_id: String,
    pub system: String,
    pub text: String,
    pub endpoint: String,
    pub model: String,
    pub fault: Option<String>,
    pub operation_id: Option<String>,
}

pub fn emit(value: &Value) {
    println!("{value}");
    std::io::stdout().flush().expect("stdout is open");
}

pub fn crash(fault: Option<&str>, boundary: &str) {
    if fault == Some(boundary) {
        std::process::exit(CRASH_EXIT);
    }
}

pub fn error(error: &agent_rs::Error) {
    let code = match error {
        agent_rs::Error::IdempotencyConflict => "idempotency_conflict",
        agent_rs::Error::StoreBusy => "store_busy",
        agent_rs::Error::ConversationBusy => "conversation_busy",
        agent_rs::Error::NotFound => "not_found",
        agent_rs::Error::Incompatible(_) => "incompatible",
        _ => "adapter_error",
    };
    emit(&json!({"event":"driver_error", "code":code}));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_unknown_action_is_rejected() {
        assert!(serde_json::from_str::<Action>("\"invented\"").is_err());
    }
    #[test]
    fn native_actions_use_the_common_protocol_names() {
        assert_eq!(
            serde_json::from_str::<Action>("\"invoke\"").unwrap(),
            Action::Invoke
        );
        assert_eq!(
            serde_json::from_str::<Action>("\"resume\"").unwrap(),
            Action::Resume
        );
        assert_eq!(
            serde_json::from_str::<Action>("\"inspect\"").unwrap(),
            Action::Inspect
        );
    }
}
