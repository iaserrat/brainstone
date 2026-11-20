#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Streams are bounded by silence, not by wall-clock length. The test build
//! shrinks `IDLE_TIMEOUT` so both cases finish in about a second.
use std::convert::Infallible;
use std::time::{Duration, Instant};

use agent::{Event, Model, Sink, StopReason, Thread};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::http::IDLE_TIMEOUT;
use crate::{CODE_TIMEOUT, ResponsesModel};

const EVENTS: [&str; 6] = [
    "data: {\"type\":\"response.output_text.delta\",\"delta\":\"b\"}\n\n",
    "data: {\"type\":\"response.output_text.delta\",\"delta\":\"on\"}\n\n",
    "data: {\"type\":\"response.output_text.delta\",\"delta\":\"jo\"}\n\n",
    "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ur\"}\n\n",
    "data: {\"type\":\"response.output_text.delta\",\"delta\":\" ☕\"}\n\n",
    "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"output\":[{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"bonjour ☕\"}]}]}}\n\n",
];

#[tokio::test]
async fn a_slow_but_continuous_stream_outlives_the_idle_timeout() {
    let endpoint = serve(EVENTS.len(), IDLE_TIMEOUT / 4).await;
    let started = Instant::now();

    let response = model(&endpoint)
        .generate_stream(&Thread::new("system", "prompt"), &[], &mut Discard)
        .await
        .unwrap();

    assert!(started.elapsed() > IDLE_TIMEOUT);
    assert_eq!(response.text, "bonjour ☕");
    assert_eq!(response.stop, StopReason::EndTurn);
}

#[tokio::test]
async fn a_stream_that_goes_silent_is_a_request_timeout() {
    let endpoint = serve(1, Duration::ZERO).await;

    let response = model(&endpoint)
        .generate_stream(&Thread::new("system", "prompt"), &[], &mut Discard)
        .await
        .unwrap();

    assert!(
        matches!(
            response.stop,
            StopReason::Error { ref code, .. } if code == CODE_TIMEOUT
        ),
        "{:?}",
        response.stop
    );
}

struct Discard;

impl Sink for Discard {
    type Error = Infallible;

    async fn emit(&mut self, _event: Event) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn model(endpoint: &str) -> ResponsesModel {
    ResponsesModel::new(endpoint, "test-model", "secret").unwrap()
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
    format!("http://{address}/v1/responses")
}
