use crate::{failure::Failure, sse::invalid};
use agent::ModelResponse;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct Output {
    message: Value,
    calls: BTreeMap<u64, Value>,
    reasoning: BTreeMap<u64, Value>,
    finish: Value,
}

impl Output {
    pub(crate) fn push(&mut self, value: &Value) -> Result<String, Failure> {
        if value.get("error").is_some() {
            return Err(crate::chat_stream::rejection(value));
        }
        let choices = value["choices"]
            .as_array()
            .ok_or_else(|| invalid("missing stream choices"))?;
        if choices.is_empty() {
            return Ok(String::new());
        }
        if choices.len() != 1 || choices[0]["index"] != 0 {
            return Err(invalid("expected choice index zero"));
        }
        if !self.finish.is_null() {
            return repeated_finish(&choices[0], &self.finish);
        }
        self.delta(&choices[0]["delta"])?;
        if !choices[0]["finish_reason"].is_null() {
            self.finish = choices[0]["finish_reason"].clone();
        }
        Ok(choices[0]["delta"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_owned())
    }

    fn delta(&mut self, delta: &Value) -> Result<(), Failure> {
        if self.message.is_null() {
            self.message = json!({"role":"assistant", "content":""});
        }
        crate::chat_stream_merge::message(&mut self.message, delta)?;
        crate::chat_stream_merge::indexed(&mut self.calls, delta.get("tool_calls"))?;
        crate::chat_stream_merge::indexed(&mut self.reasoning, delta.get("reasoning_details"))
    }

    pub(crate) fn reasoning_delta(&self, value: &Value) -> String {
        crate::chat_reasoning::visible(&value["choices"][0]["delta"], &self.reasoning)
    }

    pub(crate) fn settled(&self) -> Result<ModelResponse, Failure> {
        let mut message = self.message.clone();
        if !self.calls.is_empty() {
            message["tool_calls"] = self.calls.values().cloned().collect();
        }
        if !self.reasoning.is_empty() {
            message["reasoning_details"] = self.reasoning.values().cloned().collect();
        }
        crate::chat_response::settled(&message, &self.finish)
    }
}

// Routers may repeat the terminal choice in their final usage chunk.
fn repeated_finish(choice: &Value, finish: &Value) -> Result<String, Failure> {
    let delta = choice["delta"]
        .as_object()
        .ok_or_else(|| invalid("chat data after terminal choice"))?;
    if &choice["finish_reason"] != finish || !delta.iter().all(empty_field) {
        return Err(invalid("chat data after terminal choice"));
    }
    Ok(String::new())
}

fn empty_field((name, value): (&String, &Value)) -> bool {
    match name.as_str() {
        "role" => value == "assistant",
        "content" => value.is_null() || value == "",
        _ => false,
    }
}
