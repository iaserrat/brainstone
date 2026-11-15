//! Table-driven wire shapes are the point of this file, so cases may exceed
//! the production function budget.
#![allow(clippy::too_many_lines)]
use super::State;
use agent::{Event, Sink};
use serde_json::json;
#[derive(Default)]
struct Events(Vec<Event>);
impl Sink for Events {
    type Error = std::convert::Infallible;
    fn emit(&mut self, event: Event) -> impl Future<Output = Result<(), Self::Error>> {
        self.0.push(event);
        std::future::ready(Ok(()))
    }
}
#[tokio::test]
async fn visible_reasoning_streams_before_settlement_and_preserves_the_provider_payload() {
    let text = "évidence ".repeat(80);
    let delta = visible_delta(&text);
    let mut state = State::default();
    let mut sink = Events::default();
    chunk(&mut state, &mut sink, delta, serde_json::Value::Null).await;
    assert_eq!(
        sink.0,
        vec![Event::ReasoningDelta {
            delta: text.clone()
        }]
    );
    let response = finish(&mut state, &mut sink).await;
    assert_eq!(response.text, "answer");
    assert_eq!(
        sink.0[1],
        Event::ModelDelta {
            delta: "answer".into()
        }
    );
    settled_reasoning(&response, &text);
}
#[tokio::test]
async fn reasoning_aliases_are_not_duplicated_and_encrypted_data_is_never_displayed() {
    let mut state = State::default();
    let mut sink = Events::default();
    chunk(&mut state, &mut sink, json!({"reasoning":"visible","reasoning_content":"duplicate","reasoning_details":[{"index":0,"type":"reasoning.encrypted","data":"secret","signature":"sig"}]}), serde_json::Value::Null).await;
    state
        .flush(&mut sink)
        .await
        .unwrap_or_else(|_| panic!("stream step failed"));
    assert_eq!(
        sink.0,
        vec![Event::ReasoningDelta {
            delta: "visible".into()
        }]
    );
}
async fn chunk(
    state: &mut State,
    sink: &mut Events,
    delta: serde_json::Value,
    finish: serde_json::Value,
) {
    let event = json!({"choices":[{"index":0,"delta":delta,"finish_reason":finish}]});
    state
        .push(format!("data: {event}\n\n").as_bytes())
        .unwrap_or_else(|_| panic!("stream step failed"));
    assert!(
        state
            .events(sink)
            .await
            .unwrap_or_else(|_| panic!("stream step failed"))
            .is_none()
    );
}

fn settled_reasoning(response: &agent::ModelResponse, text: &str) {
    let agent::Reasoning::Opaque { data, .. } = &response.reasoning[0] else {
        panic!("opaque reasoning");
    };
    assert_eq!(data["reasoning_details"][0]["text"], text);
    assert_eq!(
        data["reasoning_details"][0]["signature"],
        "opaque-signature"
    );
}

fn visible_delta(text: &str) -> serde_json::Value {
    json!({"reasoning":text,"reasoning_details":[{"index":0,"type":"reasoning.text","text":text,"signature":"opaque-signature"}]})
}

async fn finish(state: &mut State, sink: &mut Events) -> agent::ModelResponse {
    chunk(state, sink, json!({"content":"answer"}), json!("stop")).await;
    state
        .push(b"data: [DONE]\n\n")
        .unwrap_or_else(|_| panic!("stream step failed"));
    state
        .events(sink)
        .await
        .unwrap_or_else(|_| panic!("stream step failed"))
        .expect("settled response")
}

#[tokio::test]
async fn a_failed_reasoning_write_stops_streaming() {
    struct Refuse;
    impl Sink for Refuse {
        type Error = &'static str;
        fn emit(&mut self, _: Event) -> impl Future<Output = Result<(), Self::Error>> {
            std::future::ready(Err("durability failed"))
        }
    }
    let mut state = State::default();
    let event =
        json!({"choices":[{"index":0,"delta":{"reasoning":"x".repeat(600)},"finish_reason":null}]});
    state
        .push(format!("data: {event}\n\n").as_bytes())
        .unwrap_or_else(|_| panic!("valid event"));
    assert!(matches!(
        state.events(&mut Refuse).await,
        Err(crate::stream::Error::Sink("durability failed"))
    ));
}

/// Each server family names visible reasoning differently; all reach one event.
#[tokio::test]
async fn every_visible_reasoning_shape_streams_as_reasoning() {
    let cases = [
        // DeepSeek and vLLM.
        vec![json!({"reasoning_content":"pensé ☕"})],
        // Router summaries.
        vec![
            json!({"reasoning_details":[{"index":0,"type":"reasoning.summary","summary":"pensé ☕"}]}),
        ],
        // A continuation item names its type only in its first chunk.
        vec![
            json!({"reasoning_details":[{"index":0,"type":"reasoning.text","text":"pensé"}]}),
            json!({"reasoning_details":[{"index":0,"text":" ☕"}]}),
        ],
    ];
    for deltas in cases {
        let mut state = State::default();
        let mut sink = Events::default();
        for delta in deltas {
            chunk(&mut state, &mut sink, delta, serde_json::Value::Null).await;
        }
        state
            .flush(&mut sink)
            .await
            .unwrap_or_else(|_| panic!("stream step failed"));
        assert_eq!(
            sink.0,
            vec![Event::ReasoningDelta {
                delta: "pensé ☕".into()
            }]
        );
    }
}
