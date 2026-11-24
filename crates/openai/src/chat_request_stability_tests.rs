//! Contract: the model-visible prefix is stable across requests, because that
//! is what a provider's prompt cache matches on. Only the message tail grows.

use agent::{Message, Thread};
use serde_json::{Value, json};

fn model() -> crate::ChatCompletionsModel {
    crate::ChatCompletionsModel::new("http://127.0.0.1:1/v1/chat/completions", "m", "k").unwrap()
}

fn thread(messages: Vec<Message>) -> Thread {
    Thread::restore("a stable system prompt", messages).unwrap()
}

fn request(thread: &Thread, tools: &[Value]) -> Value {
    match crate::chat_request::request(&model(), thread, tools) {
        Ok(value) => value,
        Err(error) => panic!("request failed: {}", error.message),
    }
}

const TOOLS: [fn() -> Value; 2] = [
    || json!({"name":"first","description":"one","input_schema":{"type":"object"}}),
    || json!({"name":"second","description":"two","input_schema":{"type":"object"}}),
];

fn tools() -> Vec<Value> {
    TOOLS.iter().map(|tool| tool()).collect()
}

#[test]
fn the_same_thread_produces_the_same_request_bytes() {
    let thread = thread(vec![Message::User("start".to_owned())]);

    assert_eq!(request(&thread, &tools()), request(&thread, &tools()));
}

#[test]
fn appending_a_message_extends_only_the_tail() {
    let before = thread(vec![Message::User("start".to_owned())]);
    let after = thread(vec![
        Message::User("start".to_owned()),
        Message::User("continue".to_owned()),
    ]);
    let first = request(&before, &tools());
    let second = request(&after, &tools());

    assert_eq!(first["system"], second["system"]);
    assert_eq!(first["tools"], second["tools"]);
    // The history put the system prompt first, then every message in order.
    assert_eq!(first["messages"][0], second["messages"][0]);
    assert_eq!(second["messages"][1]["content"], "start");
    assert_eq!(second["messages"][2]["content"], "continue");
    assert_eq!(second["messages"].as_array().unwrap().len(), 3);
}

#[test]
fn tools_keep_the_order_the_registry_gave_them() {
    let thread = thread(vec![Message::User("start".to_owned())]);
    let tools = tools();
    let request = request(&thread, &tools);
    let names: Vec<&str> = request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["function"]["name"].as_str().unwrap())
        .collect();

    assert_eq!(names, ["first", "second"]);
}
