//! Structured output contracts live in accepted profiles and validate terminal answers.
use brainstone_agent::{
    ExecutionProfile, Message, ModelResponse, Outcome, OutputSchema, Step, StopReason, Thread,
};
use serde_json::json;

fn schema() -> OutputSchema {
    OutputSchema::new("answer", json!({"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":false})).unwrap()
}

fn thread(text: &str, stop: StopReason) -> Thread {
    let profile = ExecutionProfile::default().with_output_schema(schema());
    let mut thread = Thread::new_with_profile("Return JSON", "hello", profile);
    thread.push(Message::Assistant(ModelResponse {
        reasoning: Vec::new(),
        text: text.to_owned(),
        tool_calls: Vec::new(),
        stop,
        model: None,
    }));
    thread
}

#[test]
fn valid_structured_answers_complete_and_profiles_round_trip() {
    let thread = thread("{\"value\":\"Zoë ☕\"}", StopReason::EndTurn);
    assert!(matches!(
        brainstone_agent::next(&thread),
        Step::Done(Outcome::Completed { .. })
    ));
    let restored: Thread = serde_json::from_value(serde_json::to_value(&thread).unwrap()).unwrap();
    assert_eq!(
        restored.profile().output_schema(),
        thread.profile().output_schema()
    );
    assert_eq!(
        brainstone_agent::next(&restored),
        brainstone_agent::next(&thread)
    );
}

#[test]
fn malformed_or_schema_invalid_output_never_completes_successfully() {
    for text in [
        "not JSON",
        "{\"value\":42}",
        "{}",
        "{\"value\":\"ok\",\"extra\":true}",
    ] {
        let Step::Done(Outcome::Failed { code, .. }) =
            brainstone_agent::next(&thread(text, StopReason::EndTurn))
        else {
            panic!("invalid structured output completed: {text}");
        };
        assert_eq!(code, "structured_output_invalid");
    }
}

#[test]
fn truncated_structured_output_fails_even_when_its_prefix_parses() {
    let Step::Done(Outcome::Failed { code, .. }) =
        brainstone_agent::next(&thread("{\"value\":\"ok\"}", StopReason::MaxTokens))
    else {
        panic!("truncated output completed");
    };
    assert_eq!(code, "structured_output_invalid");
}
