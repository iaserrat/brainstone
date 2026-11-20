//! Retry identity must distinguish effects from separate steered model turns.
use brainstone_agent::{Message, ModelResponse, StopReason, Thread, ToolCall};
use serde_json::json;

fn call() -> Message {
    Message::Assistant(ModelResponse {
        reasoning: Vec::new(),
        text: String::new(),
        tool_calls: vec![ToolCall {
            id: "reused-within-another-response".to_owned(),
            name: "RecordValue".to_owned(),
            args: json!({"value":"one"}),
        }],
        stop: StopReason::ToolUse,
        model: Some("test".to_owned()),
    })
}

#[test]
fn new_threads_do_not_reuse_a_tool_step_after_steering() {
    let mut thread = Thread::new("system", "first request");
    thread.push(call());
    let first = thread.tool_key_step();
    thread.push(Message::User("second request".to_owned()));
    thread.push(call());
    assert_ne!(thread.tool_key_step(), first);
}

fn legacy_profile() -> brainstone_agent::ExecutionProfile {
    serde_json::from_value(json!({
        "model":null, "effort":null, "max_turns":"unlimited", "depth":0
    }))
    .unwrap()
}

#[test]
fn missing_profile_versions_preserve_issued_legacy_keys() {
    let mut thread = Thread::new_with_profile("system", "first", legacy_profile());
    thread.push(call());
    assert_eq!(thread.tool_key_step(), Some(1));
    thread.compact("summary".to_owned());
    thread.push(Message::User("second".to_owned()));
    thread.push(call());
    let restored =
        Thread::restore_compacted("system", thread.messages().to_vec(), legacy_profile(), 1)
            .unwrap();
    assert_eq!(restored.tool_key_step(), Some(2));
    assert_eq!(restored.tool_key_step(), thread.tool_key_step());
    assert!(
        serde_json::to_value(legacy_profile())
            .unwrap()
            .get("tool_key_version")
            .is_none()
    );
}

#[test]
fn current_identity_survives_compaction_and_requires_explicit_recovery_count() {
    let mut thread = Thread::new("system", "first");
    thread.push(call());
    thread.push(Message::User("second".to_owned()));
    thread.push(call());
    thread.compact("summary".to_owned());
    thread.push(Message::User("third".to_owned()));
    thread.push(call());
    assert_eq!(thread.tool_key_step(), Some(3));
    assert_eq!(
        thread.steps(),
        1,
        "steering still resets the response budget"
    );
    let restored = Thread::restore_compacted(
        "system",
        thread.messages().to_vec(),
        thread.profile().clone(),
        1,
    )
    .unwrap();
    assert_eq!(restored.tool_key_step(), None);
    assert_eq!(restored.with_tool_sequence(3).tool_key_step(), Some(3));
}

#[test]
fn profile_versions_reject_future_keys_without_changing_model_settings() {
    let current = brainstone_agent::ExecutionProfile::default();
    assert!(current.same_settings(&legacy_profile()));
    assert_ne!(current, legacy_profile());
    let mut value = serde_json::to_value(current).unwrap();
    assert_eq!(value["tool_key_version"], "operation-v2");
    value["tool_key_version"] = json!("future-v3");
    assert!(serde_json::from_value::<brainstone_agent::ExecutionProfile>(value).is_err());
}
