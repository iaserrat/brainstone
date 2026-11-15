#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Full public-path proof that the provider and agent cooperate across a tool
//! turn. Test setup is table-heavy and exempt from production size budgets.
#![allow(clippy::too_many_lines)]

use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;

use agent::{Event, Outcome, Registry, Sink, Thread, Tool, ToolError, ToolRequest, run};
use brainstone_openai::ResponsesModel;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn the_agent_completes_a_real_provider_tool_loop() {
    let (endpoint, requests) = serve_tool_loop().await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();
    let tools = Registry::new(vec![Arc::new(Echo)]);
    let mut thread = Thread::new("Be useful.", "echo café ☕");
    let mut sink = Events::default();

    let outcome = run(&mut thread, &model, &tools, &mut sink).await.unwrap();

    assert_eq!(
        outcome,
        Outcome::Completed {
            text: "café ☕".to_owned()
        }
    );
    let requests = requests.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["stream"], true);
    assert_eq!(requests[1]["input"][2]["call_id"], "call_echo");
    assert_eq!(requests[1]["input"][2]["output"], "café ☕");
    assert_eq!(
        sink.0
            .iter()
            .filter(|event| matches!(event, Event::Appended { .. }))
            .count(),
        3
    );
    assert!(
        sink.0
            .iter()
            .any(|event| matches!(event, Event::ModelDelta { delta } if delta == "café ☕"))
    );
}

struct Echo;

impl Tool for Echo {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn definition(&self) -> Value {
        json!({
            "name": "echo", "description": "Echo text",
            "input_schema": {
                "type": "object", "properties": {"text": {"type": "string"}},
                "required": ["text"], "additionalProperties": false
            }
        })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<agent::ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            Ok(request.args["text"]
                .as_str()
                .expect("the test provider supplied text")
                .to_owned()
                .into())
        })
    }
}

#[derive(Default)]
struct Events(Vec<Event>);

impl Sink for Events {
    type Error = Infallible;

    async fn emit(&mut self, event: Event) -> Result<(), Self::Error> {
        self.0.push(event);
        Ok(())
    }
}

async fn serve_tool_loop() -> (String, tokio::task::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let replies = [
            (
                json!({"status": "completed", "output": [{
                    "type": "function_call", "call_id": "call_echo", "name": "echo",
                    "arguments": "{\"text\":\"café ☕\"}"
                }]}),
                None,
            ),
            (
                json!({"status": "completed", "output": [{
                    "type": "message", "content": [{"type": "output_text", "text": "café ☕"}]
                }]}),
                Some("café ☕"),
            ),
        ];
        let mut requests = Vec::new();
        for (reply, delta) in replies {
            let (mut stream, _) = listener.accept().await.unwrap();
            requests.push(read_request(&mut stream).await);
            write_reply(&mut stream, &reply, delta).await;
        }
        requests
    });
    (format!("http://{address}/v1/responses"), task)
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> Value {
    let mut bytes = Vec::new();
    let mut chunk = [0; 1024];
    loop {
        let count = stream.read(&mut chunk).await.unwrap();
        bytes.extend_from_slice(&chunk[..count]);
        let text = std::str::from_utf8(&bytes).unwrap();
        let Some(start) = text.find("\r\n\r\n").map(|index| index + 4) else {
            continue;
        };
        let length = text.lines().find_map(content_length).unwrap();
        if bytes.len() >= start + length {
            return serde_json::from_slice(&bytes[start..start + length]).unwrap();
        }
    }
}

fn content_length(line: &str) -> Option<usize> {
    line.to_ascii_lowercase()
        .strip_prefix("content-length: ")?
        .parse()
        .ok()
}

async fn write_reply(stream: &mut tokio::net::TcpStream, response: &Value, delta: Option<&str>) {
    let progress = delta.map_or_else(String::new, |delta| {
        format!(
            "data: {}\n\n",
            json!({"type": "response.output_text.delta", "delta": delta})
        )
    });
    let terminal = json!({"type": "response.completed", "response": response});
    let body = format!("{progress}data: {terminal}\n\n");
    let head = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body.as_bytes()).await.unwrap();
}
