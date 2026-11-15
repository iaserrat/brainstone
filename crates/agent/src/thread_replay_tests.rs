use std::borrow::Cow;

use serde_json::{Value, json};

use crate::{
    CODE_UNANSWERED_CALL, Message, ModelResponse, Reasoning, StopReason, Thread, ToolCall,
    ToolResult,
};

const PRODUCER: &str = "anthropic/claude-a";
const OTHER: &str = "openai/gpt-b";

#[test]
fn a_history_its_producer_wrote_is_replayed_unchanged() {
    let thread = thread(vec![
        assistant(Some(PRODUCER), &["c1"], StopReason::ToolUse),
        results(&["c1"]),
    ]);

    let replayed = thread.replay(PRODUCER);

    assert!(matches!(replayed, Cow::Borrowed(_)));
}

#[test]
fn reasoning_another_or_an_unknown_model_wrote_is_removed() {
    for producer in [Some(OTHER), None] {
        let thread = thread(vec![
            assistant(producer, &["c1"], StopReason::ToolUse),
            results(&["c1"]),
        ]);

        let replayed = thread.replay(PRODUCER);

        let response = assistant_at(&replayed, 1);
        assert!(response.reasoning.is_empty(), "{producer:?}");
        assert_eq!(response.text, "sí, “hecho” ✓", "{producer:?}");
        assert_eq!(response.tool_calls.len(), 1, "{producer:?}");
    }
}

#[test]
fn a_failed_response_is_not_replayed() {
    let failed = StopReason::Error {
        code: "refused".to_owned(),
        message: "refused mid-stream".to_owned(),
    };
    let thread = thread(vec![
        assistant(Some(PRODUCER), &["c1"], failed),
        Message::User("otra vez".to_owned()),
    ]);

    let replayed = thread.replay(PRODUCER);

    let expected = vec![
        Message::User("hola".to_owned()),
        Message::User("otra vez".to_owned()),
    ];
    assert_eq!(replayed.messages(), expected);
}

/// A subagent stopped mid-batch and then messaged again leaves exactly this.
#[test]
fn an_unanswered_batch_is_answered_with_errors_before_the_next_turn() {
    let thread = thread(vec![
        assistant(Some(PRODUCER), &["c1", "c2"], StopReason::ToolUse),
        Message::User("otra vez".to_owned()),
    ]);

    let replayed = thread.replay(PRODUCER);

    let Message::ToolResults(results) = &replayed.messages()[2] else {
        panic!("the batch must be answered next: {:?}", replayed.messages());
    };
    assert_eq!(ids(results), ["c1", "c2"]);
    assert!(results.iter().all(unanswered));
    assert_eq!(replayed.messages()[3], Message::User("otra vez".to_owned()));
}

#[test]
fn a_partly_answered_batch_is_completed_in_place() {
    let thread = thread(vec![
        assistant(Some(PRODUCER), &["c1", "c2"], StopReason::ToolUse),
        results(&["c1"]),
    ]);

    let replayed = thread.replay(PRODUCER);

    let Message::ToolResults(results) = &replayed.messages()[2] else {
        panic!("the batch stays where it was");
    };
    assert_eq!(ids(results), ["c1", "c2"]);
    assert!(!unanswered(&results[0]));
    assert!(unanswered(&results[1]));
    assert_eq!(replayed.messages().len(), 3);
}

fn thread(rest: Vec<Message>) -> Thread {
    let mut messages = vec![Message::User("hola".to_owned())];
    messages.extend(rest);
    Thread::restore("s", messages).unwrap()
}

fn assistant(model: Option<&str>, calls: &[&str], stop: StopReason) -> Message {
    Message::Assistant(ModelResponse {
        reasoning: vec![Reasoning::Thinking {
            text: String::new(),
            signature: Some("sig".to_owned()),
        }],
        text: "sí, “hecho” ✓".to_owned(),
        tool_calls: calls
            .iter()
            .map(|id| ToolCall {
                id: (*id).to_owned(),
                name: "lookup".to_owned(),
                args: json!({"q": "ñ"}),
            })
            .collect(),
        stop,
        model: model.map(str::to_owned),
    })
}

fn results(answered: &[&str]) -> Message {
    let results = answered
        .iter()
        .map(|id| ToolResult {
            id: (*id).to_owned(),
            name: "lookup".to_owned(),
            content: "encontrado ✓".to_owned(),
            is_error: false,
            images: Vec::new(),
        })
        .collect();
    Message::ToolResults(results)
}

fn assistant_at(thread: &Thread, index: usize) -> &ModelResponse {
    let Message::Assistant(response) = &thread.messages()[index] else {
        panic!("message {index} is not an assistant response");
    };
    response
}

fn ids(results: &[ToolResult]) -> Vec<&str> {
    results.iter().map(|result| result.id.as_str()).collect()
}

fn unanswered(result: &ToolResult) -> bool {
    let document: Value = serde_json::from_str(&result.content).unwrap_or_default();
    result.is_error && result.name == "lookup" && document["error"] == CODE_UNANSWERED_CALL
}
