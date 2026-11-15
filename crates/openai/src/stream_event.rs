use serde_json::Value;

use crate::failure::{CODE_PROVIDER_FAILED, Failure};
use crate::response::Response;
use crate::sse::invalid;

pub(crate) enum Item {
    Delta(String),
    /// Provider-visible reasoning: a summary, or raw text a server chose to return.
    Reasoning(String),
    Done(Response),
    Ignore,
}

pub(crate) fn parse(data: &str) -> Result<Item, Failure> {
    if data == "[DONE]" {
        return Ok(Item::Ignore);
    }
    let value: Value = serde_json::from_str(data)
        .map_err(|error| invalid(format!("invalid stream event: {error}")))?;
    match value["type"].as_str() {
        Some("response.output_text.delta") => delta(&value).map(Item::Delta),
        Some("response.reasoning_summary_text.delta" | "response.reasoning_text.delta") => {
            delta(&value).map(Item::Reasoning)
        }
        Some("response.completed" | "response.incomplete" | "response.failed") => done(value),
        Some("error") => Err(Failure::new(
            crate::failure::classified(value["code"].as_str(), CODE_PROVIDER_FAILED),
            message(&value),
        )),
        Some(_) => Ok(Item::Ignore),
        None => Err(invalid("stream event has no type")),
    }
}

fn delta(value: &Value) -> Result<String, Failure> {
    value["delta"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("text delta is not a string"))
}

fn done(mut value: Value) -> Result<Item, Failure> {
    let response = value
        .get_mut("response")
        .map(Value::take)
        .ok_or_else(|| invalid("terminal event has no response"))?;
    serde_json::from_value::<Response>(response)
        .map(Item::Done)
        .map_err(|error| invalid(error.to_string()))
}

fn message(value: &Value) -> String {
    value["message"]
        .as_str()
        .unwrap_or("the provider stream failed")
        .to_owned()
}
