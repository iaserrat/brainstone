//! Token accounting as the router reports it.
//!
//! `usage` arrives in its own chunk at the end of a stream. Which field carries
//! the cache count depends on the provider behind the router, so both spellings
//! are read. A chunk without `usage`, or one this module cannot read, reports
//! nothing rather than zeroes: a missing number is not a cache miss.

use agent::Event;
use serde_json::Value;

pub(crate) fn event(value: &Value) -> Option<Event> {
    let usage = value.get("usage")?.as_object()?;
    let prompt_tokens = number(usage.get("prompt_tokens")?)?;
    let completion_tokens = usage.get("completion_tokens").and_then(number).unwrap_or(0);
    let cached_tokens = cache(usage).unwrap_or(0);
    Some(Event::Usage {
        prompt_tokens,
        completion_tokens,
        cached_tokens,
    })
}

/// `prompt_tokens_details.cached_tokens` (OpenAI-compatible) or
/// `cache_read_input_tokens` (Anthropic-compatible routers).
fn cache(usage: &serde_json::Map<String, Value>) -> Option<u64> {
    usage
        .get("prompt_tokens_details")
        .and_then(|details| details.get("cached_tokens"))
        .and_then(number)
        .or_else(|| usage.get("cache_read_input_tokens").and_then(number))
}

fn number(value: &Value) -> Option<u64> {
    value.as_u64()
}

#[cfg(test)]
#[path = "chat_usage_tests.rs"]
mod tests;
