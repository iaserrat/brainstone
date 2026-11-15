use serde_json::Value;

use crate::failure::Failure;
use crate::sse::invalid;

pub(crate) enum Item {
    Start { index: usize, block: Value },
    Delta { index: usize, delta: Value },
    BlockStop(usize),
    StopReason(String),
    Error(Failure),
    Done,
    Ignore,
}

pub(crate) fn parse(data: &str) -> Result<Item, Failure> {
    let value: Value = serde_json::from_str(data).map_err(|error| invalid(error.to_string()))?;
    match value["type"].as_str() {
        Some("content_block_start") => Ok(Item::Start {
            index: index(&value)?,
            block: required(&value, "content_block")?,
        }),
        Some("content_block_delta") => Ok(Item::Delta {
            index: index(&value)?,
            delta: required(&value, "delta")?,
        }),
        Some("content_block_stop") => Ok(Item::BlockStop(index(&value)?)),
        Some("message_delta") => value["delta"]["stop_reason"]
            .as_str()
            .map(|reason| Item::StopReason(reason.to_owned()))
            .ok_or_else(|| invalid("message delta has no stop reason")),
        Some("message_stop") => Ok(Item::Done),
        Some("error") => Ok(Item::Error(api_error(&value))),
        Some(_) => Ok(Item::Ignore),
        None => Err(invalid("stream event has no type")),
    }
}

fn api_error(value: &Value) -> Failure {
    let kind = value["error"]["type"].as_str().unwrap_or("unknown_error");
    let code = match kind {
        "invalid_request_error" => crate::INVALID_REQUEST,
        "authentication_error" => crate::AUTHENTICATION_FAILED,
        "billing_error" => crate::BILLING_ERROR,
        "permission_error" => crate::PERMISSION_DENIED,
        "not_found_error" => crate::NOT_FOUND,
        "conflict_error" => crate::CONFLICT,
        "rate_limit_error" => crate::RATE_LIMITED,
        "api_error" | "overloaded_error" => crate::PROVIDER_UNAVAILABLE,
        _ => crate::PROVIDER_FAILED,
    };
    let message = value["error"]["message"]
        .as_str()
        .unwrap_or("the Anthropic stream failed");
    Failure::new(code, message)
}

fn index(value: &Value) -> Result<usize, Failure> {
    value["index"]
        .as_u64()
        .and_then(|index| usize::try_from(index).ok())
        .ok_or_else(|| invalid("content event has no valid index"))
}

fn required(value: &Value, field: &str) -> Result<Value, Failure> {
    value
        .get(field)
        .cloned()
        .ok_or_else(|| invalid(format!("stream event has no {field}")))
}
