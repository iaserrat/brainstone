//! Table-driven wire contracts cover optional schemas on both `OpenAI` protocols.
#![allow(clippy::too_many_lines)]

use agent::{ExecutionProfile, OutputSchema, Thread};
use serde_json::{Value, json};

use crate::{ChatCompletionsModel, ResponsesModel};

fn schema() -> Value {
    json!({"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":false})
}

fn thread() -> Thread {
    let output = OutputSchema::new("answer", schema()).unwrap();
    let profile = ExecutionProfile::new(None, Some(agent::Effort::High), 4, 0)
        .unwrap()
        .with_output_schema(output);
    Thread::new_with_profile("system", "prompt", profile)
}

fn tools() -> Vec<Value> {
    vec![json!({"name":"lookup","description":"Read a value","input_schema":{"type":"object"}})]
}

#[test]
fn chat_schema_and_tool_definitions_are_independent() {
    let model =
        ChatCompletionsModel::new("http://localhost/chat/completions", "model", "secret").unwrap();
    let body = crate::chat_request::request(&model, &thread(), &tools())
        .ok()
        .unwrap();
    assert_eq!(
        body["response_format"],
        json!({"type":"json_schema","json_schema":{"name":"answer","schema":schema(),"strict":true}})
    );
    assert_eq!(body["tools"].as_array().unwrap().len(), 1);
    assert!(body.get("tool_choice").is_none());
    assert_eq!(body["reasoning_effort"], "high");
}

#[test]
fn responses_schema_is_preserved_in_streaming_and_settled_requests() {
    let model = ResponsesModel::new("http://localhost/responses", "model", "secret").unwrap();
    let thread = thread();
    for streaming in [false, true] {
        let request = crate::request::Request::new(&model, &thread, &tools())
            .ok()
            .unwrap();
        let request = if streaming {
            request.streaming()
        } else {
            request
        };
        let body = serde_json::to_value(request).unwrap();
        assert_eq!(
            body["text"],
            json!({"format":{"type":"json_schema","name":"answer","schema":schema(),"strict":true}})
        );
        assert_eq!(body["tools"].as_array().unwrap().len(), 1);
        assert!(body.get("tool_choice").is_none());
        assert_eq!(body["reasoning"]["effort"], "high");
        assert_eq!(
            body.get("stream").and_then(Value::as_bool).unwrap_or(false),
            streaming
        );
    }
}

#[test]
fn omitted_schema_keeps_the_existing_request_shapes() {
    let thread = Thread::new("system", "prompt");
    let chat =
        ChatCompletionsModel::new("http://localhost/chat/completions", "model", "secret").unwrap();
    let chat = crate::chat_request::request(&chat, &thread, &[])
        .ok()
        .unwrap();
    assert!(chat.get("response_format").is_none());
    let responses = ResponsesModel::new("http://localhost/responses", "model", "secret").unwrap();
    let body = serde_json::to_value(
        crate::request::Request::new(&responses, &thread, &[])
            .ok()
            .unwrap(),
    )
    .unwrap();
    assert!(body.get("text").is_none());
}
