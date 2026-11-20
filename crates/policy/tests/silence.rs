//! `Silence` is only useful if the name it advertises is the name the loop
//! ends a run on. Nothing else in the workspace holds both halves, so the
//! drift guard lives here.

use std::sync::Arc;

use agent::{Message, Outcome, Registry, Step, ToolCall, execute_tool, next};
use serde_json::json;

#[tokio::test]
async fn calling_the_advertised_silence_tool_ends_the_run_with_no_text() {
    let registry = Registry::new(vec![Arc::new(brainstone_policy::Silence)]);
    let definitions = registry.definitions();
    let name = definitions[0]["name"].as_str().expect("a name");
    let call = ToolCall {
        id: "c1".to_owned(),
        name: name.to_owned(),
        args: json!({}),
    };
    let result = execute_tool(&call, &registry, None).await;
    assert!(!result.is_error, "the advertised name must dispatch");

    let mut thread = agent::Thread::new("s", "not for you");
    thread.push(Message::Assistant(agent::ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: String::new(),
        tool_calls: vec![call],
        stop: agent::StopReason::ToolUse,
    }));
    thread.push(Message::ToolResults(vec![result]));

    assert_eq!(
        next(&thread),
        Step::Done(Outcome::Completed {
            text: String::new()
        })
    );
}
