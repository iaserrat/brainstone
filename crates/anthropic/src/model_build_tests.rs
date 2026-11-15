//! Streams are bounded by silence, not by wall-clock length. The test build
//! shrinks `IDLE_TIMEOUT` so both cases finish in about a second.
use std::time::{Duration, Instant};

use agent::{Model, StopReason, Thread};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::model_build::IDLE_TIMEOUT;
use crate::{AnthropicModel, Auth, Endpoint, REQUEST_TIMEOUT};

const EVENTS: [&str; 7] = [
    "data: {\"type\":\"message_start\",\"message\":{\"content\":[],\"stop_reason\":null}}\n\n",
    "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"bonjour \"}}\n\n",
    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"☕\"}}\n\n",
    "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n",
    "data: {\"type\":\"message_stop\"}\n\n",
];

#[tokio::test]
async fn a_slow_but_continuous_stream_outlives_the_idle_timeout() {
    let endpoint = serve(EVENTS.len(), IDLE_TIMEOUT / 4).await;
    let started = Instant::now();

    let response = model(&endpoint)
        .generate(&Thread::new("system", "prompt"), &[])
        .await;

    assert!(started.elapsed() > IDLE_TIMEOUT);
    assert_eq!(response.text, "bonjour ☕");
    assert_eq!(response.stop, StopReason::EndTurn);
}

#[tokio::test]
async fn a_stream_that_goes_silent_is_a_request_timeout() {
    let endpoint = serve(1, Duration::ZERO).await;

    let response = model(&endpoint)
        .generate(&Thread::new("system", "prompt"), &[])
        .await;

    assert!(
        matches!(
            response.stop,
            StopReason::Error { ref code, .. } if code == REQUEST_TIMEOUT
        ),
        "{:?}",
        response.stop
    );
}

fn model(endpoint: &str) -> AnthropicModel {
    let endpoint = Endpoint::messages(endpoint).unwrap();
    AnthropicModel::new(
        endpoint,
        "claude-test",
        Auth::ApiKey("secret".to_owned()),
        4096,
    )
    .unwrap()
}

/// Announces the whole body, writes the first `sent` events `gap` apart, then
/// holds the socket open until the client hangs up.
async fn serve(sent: usize, gap: Duration) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let length: usize = EVENTS.iter().map(|event| event.len()).sum();
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {length}\r\n\r\n"
        );
        stream.write_all(head.as_bytes()).await.unwrap();
        for event in &EVENTS[..sent] {
            tokio::time::sleep(gap).await;
            stream.write_all(event.as_bytes()).await.unwrap();
        }
        while stream
            .read(&mut [0; 1024])
            .await
            .is_ok_and(|count| count > 0)
        {}
    });
    format!("http://{address}/v1/messages")
}
