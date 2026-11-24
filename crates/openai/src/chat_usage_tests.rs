//! Contract: token accounting arrives in its own chunk, and a cache count is
//! read from either spelling. A chunk without usage reports nothing.

use agent::Event;
use serde_json::{Value, json};

fn event(value: &Value) -> Option<Event> {
    crate::chat_usage::event(value)
}

#[test]
fn reads_the_openai_compatible_cache_count() {
    let value = json!({"choices": [], "usage": {
        "prompt_tokens": 4_096,
        "completion_tokens": 120,
        "prompt_tokens_details": {"cached_tokens": 3_840},
    }});

    assert_eq!(
        event(&value),
        Some(Event::Usage {
            prompt_tokens: 4_096,
            completion_tokens: 120,
            cached_tokens: 3_840,
        })
    );
}

#[test]
fn reads_the_anthropic_compatible_cache_count() {
    let value = json!({"usage": {"prompt_tokens": 900, "cache_read_input_tokens": 512}});

    assert_eq!(
        event(&value),
        Some(Event::Usage {
            prompt_tokens: 900,
            completion_tokens: 0,
            cached_tokens: 512,
        })
    );
}

#[test]
fn a_chunk_without_usage_reports_nothing() {
    let delta = json!({"choices": [{"index": 0, "delta": {"content": "hi"}}]});
    let null_prompt = json!({"usage": {"prompt_tokens": null}});
    let empty = json!({"usage": {}});

    assert_eq!(event(&delta), None);
    assert_eq!(event(&null_prompt), None);
    assert_eq!(event(&empty), None);
}

#[test]
fn a_reported_cache_hit_of_zero_is_still_a_report() {
    let value = json!({"usage": {"prompt_tokens": 10, "completion_tokens": 1}});

    assert_eq!(
        event(&value),
        Some(Event::Usage {
            prompt_tokens: 10,
            completion_tokens: 1,
            cached_tokens: 0,
        })
    );
}
