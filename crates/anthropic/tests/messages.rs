#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Public Anthropic HTTP behavior is table-heavy and exempt from production
//! source budgets.
#![allow(clippy::too_many_lines)]

use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;

use agent::{
    Effort as AgentEffort, Event, ExecutionProfile, Image, ImageMediaType, Message, Model,
    Registry, Sink, StopReason, Thread, Tool, ToolError, ToolRequest, UserImages, run,
};
use aws_credential_types::Credentials;
use aws_credential_types::provider::SharedCredentialsProvider;
use aws_smithy_eventstream::frame::write_message_to;
use aws_smithy_types::event_stream::{Header, HeaderValue, Message as Frame};
use base64::Engine;
use brainstone_anthropic::{AnthropicModel, Auth, ConfigError, Effort, Endpoint, SigV4};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn text_streams_through_the_native_messages_contract() {
    let (endpoint, captured) = serve(sse_text()).await;
    let model = AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096)
        .unwrap()
        .with_effort(Effort::XHigh);
    let mut sink = Events::default();
    let response = model
        .generate_stream(&Thread::new("Be useful.", "héllo ☕"), &[], &mut sink)
        .await
        .unwrap();

    assert_eq!(response.text, "bonjour ☕");
    assert_eq!(response.stop, StopReason::EndTurn);
    assert_eq!(
        sink.0,
        vec![Event::ModelDelta {
            delta: "bonjour ☕".to_owned()
        }]
    );
    let request = captured.await.unwrap();
    assert!(
        request
            .head
            .to_ascii_lowercase()
            .contains("x-api-key: secret")
    );
    assert!(
        request
            .head
            .to_ascii_lowercase()
            .contains("anthropic-version: 2023-06-01")
    );
    assert_eq!(request.body["model"], "claude-test");
    assert_eq!(request.body["max_tokens"], 4096);
    assert_eq!(request.body["stream"], true);
    assert_eq!(request.body["output_config"]["effort"], "xhigh");
    assert_eq!(request.body["system"], "Be useful.");
    assert_eq!(request.body["messages"][0]["role"], "user");
    assert_eq!(
        request.body["messages"][0]["content"][0]["text"],
        "héllo ☕"
    );
}

#[tokio::test]
async fn prompt_cache_is_off_unless_enabled() {
    let (endpoint, captured) = serve(sse_text()).await;
    let model =
        AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096).unwrap();
    model.generate(&Thread::new("Be useful.", "hi"), &[]).await;
    assert!(captured.await.unwrap().body.get("cache_control").is_none());

    let (endpoint, captured) = serve(sse_text()).await;
    let model = AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096)
        .unwrap()
        .with_prompt_cache();
    model.generate(&Thread::new("Be useful.", "hi"), &[]).await;
    assert_eq!(
        captured.await.unwrap().body["cache_control"],
        serde_json::json!({"type": "ephemeral"})
    );
}

#[tokio::test]
async fn images_precede_optional_text_in_messages() {
    let (endpoint, captured) = serve(sse_text()).await;
    let model =
        AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096).unwrap();
    let image = Image::new(ImageMediaType::Png, vec![1, 2, 3], 2, 3).unwrap();
    let thread = Thread::restore(
        "system",
        vec![Message::UserImages(
            UserImages::new("", vec![image]).unwrap(),
        )],
    )
    .unwrap();
    model.generate(&thread, &[]).await;
    let body = captured.await.unwrap().body;
    let content = &body["messages"][0]["content"];
    assert_eq!(content[0]["type"], "image");
    assert_eq!(content[0]["source"]["media_type"], "image/png");
    assert_eq!(content[0]["source"]["data"], "AQID");
    assert_eq!(content.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn execution_profile_overrides_parent_model_and_effort() {
    let (endpoint, captured) = serve(sse_text()).await;
    let model = AnthropicModel::new(messages(&endpoint), "claude-parent", key("secret"), 4096)
        .unwrap()
        .with_effort(Effort::XHigh);
    let profile =
        ExecutionProfile::new(Some("claude-sonnet-5"), Some(AgentEffort::Low), 4, 1).unwrap();

    model
        .generate(&Thread::new_with_profile("system", "task", profile), &[])
        .await;

    let request = captured.await.unwrap();
    assert_eq!(request.body["model"], "claude-sonnet-5");
    assert_eq!(request.body["output_config"]["effort"], "low");
}

#[tokio::test]
async fn tool_result_attachments_replay_as_blocks_inside_the_result() {
    let (endpoint, captured) = serve(sse_text()).await;
    let model =
        AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096).unwrap();
    let image = Image::new(ImageMediaType::Png, vec![9], 1, 1).unwrap();
    let call = agent::ToolCall {
        id: "toolu_1".to_owned(),
        name: "ReadThreadFile".to_owned(),
        args: serde_json::json!({}),
    };
    let thread = Thread::restore(
        "system",
        vec![
            Message::User("read it".to_owned()),
            Message::Assistant(agent::ModelResponse {
                model: None,
                reasoning: Vec::new(),
                text: String::new(),
                tool_calls: vec![call],
                stop: StopReason::ToolUse,
            }),
            Message::ToolResults(vec![agent::ToolResult {
                id: "toolu_1".to_owned(),
                name: "ReadThreadFile".to_owned(),
                content: r#"{"outcome":"file_read"}"#.to_owned(),
                is_error: false,
                images: vec![image],
            }]),
        ],
    )
    .unwrap();
    model.generate(&thread, &[]).await;
    let body = captured.await.unwrap().body;
    let content = &body["messages"][2]["content"][0]["content"];
    assert_eq!(content[0]["type"], "image");
    assert_eq!(content[0]["source"]["data"], "CQ==");
    assert_eq!(content[1]["type"], "text");
    assert_eq!(content[1]["text"], r#"{"outcome":"file_read"}"#);
    assert_eq!(content.as_array().unwrap().len(), 2);
}

/// Messages accepts tool ids matching `^[a-zA-Z0-9_-]+$` only; another
/// provider's id may not. The mapping is one-to-one, so ids that differ before
/// it differ after it, and each call still pairs with its own result.
#[tokio::test]
async fn a_foreign_tool_id_replays_one_to_one_in_the_alphabet_messages_accepts() {
    let (endpoint, captured) = serve(sse_text()).await;
    let model =
        AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096).unwrap();
    let ids = ["call_a|b.ç", "a.b", "a,b", "a-2eb", "toolu_01AB", ""];
    let call = |id: &str| agent::ToolCall {
        id: id.to_owned(),
        name: "lookup".to_owned(),
        args: serde_json::json!({}),
    };
    let result = |id: &str| agent::ToolResult {
        id: id.to_owned(),
        name: "lookup".to_owned(),
        content: "found".to_owned(),
        is_error: false,
        images: Vec::new(),
    };
    let thread = Thread::restore(
        "system",
        vec![
            Message::User("look".to_owned()),
            Message::Assistant(agent::ModelResponse {
                model: None,
                reasoning: Vec::new(),
                text: String::new(),
                tool_calls: ids.iter().map(|id| call(id)).collect(),
                stop: StopReason::ToolUse,
            }),
            Message::ToolResults(ids.iter().map(|id| result(id)).collect()),
        ],
    )
    .unwrap();
    model.generate(&thread, &[]).await;
    let body = captured.await.unwrap().body;
    let sent = [
        "call_a-7cb-2e-c3-a7",
        "a-2eb",
        "a-2cb",
        "a--2eb",
        "toolu_01AB",
        "-",
    ];
    for (index, id) in sent.iter().enumerate() {
        assert_eq!(body["messages"][1]["content"][index]["id"], *id);
        assert_eq!(body["messages"][2]["content"][index]["tool_use_id"], *id);
    }
}

#[tokio::test]
async fn signed_thinking_and_tools_round_trip_through_the_agent() {
    let (endpoint, captured) = serve_many(vec![sse_tool(), sse_answer()]).await;
    let model =
        AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096).unwrap();
    let tools = Registry::new(vec![Arc::new(Echo)]);
    let mut thread = Thread::new("Be useful.", "echo café ☕");
    let mut sink = Events::default();

    let outcome = run(&mut thread, &model, &tools, &mut sink).await.unwrap();

    assert_eq!(
        outcome,
        agent::Outcome::Completed {
            text: "café ☕".to_owned()
        }
    );
    let reasoning: Vec<&Event> = sink
        .0
        .iter()
        .filter(|event| matches!(event, Event::ReasoningDelta { .. }))
        .collect();
    assert_eq!(
        reasoning,
        vec![&Event::ReasoningDelta {
            delta: "consider echo".to_owned()
        }]
    );
    let requests = captured.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].body.get("output_config").is_none());
    assert_eq!(
        requests[0].body["thinking"],
        serde_json::json!({"type": "adaptive", "display": "summarized"})
    );
    let assistant = &requests[1].body["messages"][1]["content"];
    assert_eq!(
        assistant[0],
        serde_json::json!({
            "type": "thinking", "thinking": "consider echo", "signature": "sig-1"
        })
    );
    assert_eq!(
        assistant[1],
        serde_json::json!({
            "type": "redacted_thinking", "data": "opaque-1"
        })
    );
    assert_eq!(assistant[2]["type"], "tool_use");
    assert_eq!(assistant[2]["input"]["text"], "café ☕");
    let results = &requests[1].body["messages"][2]["content"];
    assert_eq!(results[0]["type"], "tool_result");
    assert_eq!(results[0]["tool_use_id"], "toolu_echo");
    assert_eq!(results[0]["content"], "café ☕");
}

#[tokio::test]
async fn transient_statuses_retry_and_terminal_statuses_are_typed() {
    let replies = vec![
        Reply::error(429, "rate_limit_error"),
        Reply::error(529, "overloaded_error"),
        Reply::stream(sse_answer()),
    ];
    let (endpoint, requests) = serve_replies(replies).await;
    let model =
        AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096).unwrap();
    let response = model.generate(&Thread::new("system", "prompt"), &[]).await;

    assert_eq!(response.text, "café ☕");
    assert_eq!(requests.await.unwrap(), 3);

    let cases = [
        (401, "authentication_failed"),
        (402, "billing_error"),
        (403, "permission_denied"),
    ];
    for (status, code) in cases {
        let (endpoint, _) = serve_replies(vec![Reply::error(status, "failure")]).await;
        let model =
            AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096).unwrap();
        let response = model.generate(&Thread::new("system", "prompt"), &[]).await;
        assert!(
            matches!(response.stop, StopReason::Error { code: ref actual, .. } if actual == code)
        );
    }
}

#[tokio::test]
async fn midstream_errors_are_typed_after_unknown_events() {
    let body = [
        r#"data: {"type":"message_start","message":{"content":[],"stop_reason":null}}\n\n"#,
        r#"data: {"type":"future_event","new_field":true}\n\n"#,
        r#"data: {"type":"error","error":{"type":"overloaded_error","message":"busy"}}\n\n"#,
    ]
    .concat()
    .replace("\\n", "\n");
    let (endpoint, _) = serve(body).await;
    let model =
        AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096).unwrap();

    let response = model.generate(&Thread::new("system", "prompt"), &[]).await;

    assert!(matches!(
        response.stop,
        StopReason::Error { ref code, ref message }
            if code == "provider_unavailable" && message == "busy"
    ));
}

/// Recorded from Bedrock: Anthropic reports an overlong prompt as an ordinary
/// invalid request, told apart only by this message.
const OVERLONG: &str = "prompt is too long: 2432184 tokens > 1000000 maximum";

fn stop_code(response: &agent::ModelResponse) -> &str {
    match &response.stop {
        StopReason::Error { code, .. } => code,
        stop => panic!("expected an error stop, got {stop:?}"),
    }
}

/// The one invalid request a caller can repair, by compacting, keeps its own
/// code on every path the rejection arrives by; every other stays generic.
#[tokio::test]
async fn an_overlong_prompt_is_typed_apart_from_other_invalid_requests() {
    let thread = Thread::new("system", "prompt");
    for (message, expected) in [
        (OVERLONG, "context_window_exceeded"),
        (
            "input length and `max_tokens` exceed context limit: 188240 + 21333 > 200000",
            "context_window_exceeded",
        ),
        ("messages.0.content: field required", "invalid_request"),
    ] {
        let reply = Reply::failure(400, "invalid_request_error", message);
        let (endpoint, _) = serve_replies(vec![reply]).await;
        let model =
            AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096).unwrap();
        assert_eq!(stop_code(&model.generate(&thread, &[]).await), expected);
    }

    let event = serde_json::json!({
        "type": "error", "error": {"type": "invalid_request_error", "message": OVERLONG}
    });
    let (endpoint, _) = serve(format!("data: {event}\n\n")).await;
    let model =
        AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096).unwrap();
    let response = model.generate(&thread, &[]).await;
    assert_eq!(stop_code(&response), "context_window_exceeded");

    let (base, _) = serve_frames(vec![exception_frame("validationException", OVERLONG)]).await;
    let credentials = Credentials::new("AKIDEXAMPLE", "secret", None, None, "test");
    let sigv4 = SigV4::with_provider("us-west-2", SharedCredentialsProvider::new(credentials));
    let endpoint =
        Endpoint::bedrock_runtime("us-west-2", Some(&base), &Auth::SigV4(sigv4.clone())).unwrap();
    let model =
        AnthropicModel::new(endpoint, "us.anthropic.claude-test", Auth::SigV4(sigv4), 64).unwrap();
    let response = model.generate(&thread, &[]).await;
    assert_eq!(stop_code(&response), "context_window_exceeded");
    let StopReason::Error { message, .. } = response.stop else {
        unreachable!("checked above");
    };
    assert_eq!(
        message, OVERLONG,
        "the provider's detail survives for humans"
    );
}

#[tokio::test]
async fn a_tool_call_cut_off_by_max_tokens_is_a_max_tokens_stop() {
    let body = [
        r#"data: {"type":"message_start","message":{"content":[],"stop_reason":null}}\n\n"#,
        r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_cut","name":"search","input":{}}}\n\n"#,
        r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"query\":\"how does the refill fl"}}\n\n"#,
        r#"data: {"type":"content_block_stop","index":0}\n\n"#,
        r#"data: {"type":"message_delta","delta":{"stop_reason":"max_tokens"}}\n\n"#,
        r#"data: {"type":"message_stop"}\n\n"#,
    ]
    .concat()
    .replace("\\n", "\n");
    let (endpoint, _) = serve(body).await;
    let model =
        AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096).unwrap();

    let response = model.generate(&Thread::new("system", "prompt"), &[]).await;

    assert_eq!(response.stop, StopReason::MaxTokens);
    assert_eq!(
        response.tool_calls.len(),
        1,
        "the truncated call is kept for refusal"
    );
    assert_eq!(response.tool_calls[0].id, "toolu_cut");
}

#[tokio::test]
async fn a_stream_cut_before_message_stop_is_a_transport_error() {
    let body = sse_text().replace(r#"data: {"type":"message_stop"}"#, ": keepalive");
    let (endpoint, _) = serve(body).await;
    let model =
        AnthropicModel::new(messages(&endpoint), "claude-test", key("secret"), 4096).unwrap();

    let response = model.generate(&Thread::new("system", "prompt"), &[]).await;

    assert!(
        matches!(
            response.stop,
            StopReason::Error { ref code, .. } if code == "transport_error"
        ),
        "{:?}",
        response.stop
    );
}

#[tokio::test]
async fn configuration_and_tool_failures_are_stable_and_secret_safe() {
    let error = Endpoint::messages("file:///tmp/messages").unwrap_err();
    assert_eq!(error.code(), "invalid_endpoint");
    assert_eq!(
        AnthropicModel::new(anthropic_api(), "", key("secret"), 1).unwrap_err(),
        ConfigError::EmptyModel
    );
    assert_eq!(
        AnthropicModel::new(anthropic_api(), "model", key(""), 1).unwrap_err(),
        ConfigError::EmptyApiKey
    );
    assert_eq!(
        AnthropicModel::new(anthropic_api(), "model", key("secret"), 0).unwrap_err(),
        ConfigError::InvalidMaxTokens
    );
    let model = AnthropicModel::new(
        messages("http://127.0.0.1:1/v1/messages"),
        "model",
        key("secret"),
        1,
    )
    .unwrap();
    assert_eq!(
        Endpoint::bedrock_mantle(" ").unwrap_err(),
        ConfigError::EmptyRegion
    );
    assert_eq!(
        Endpoint::bedrock_runtime("us-east-1", None, &key("")).unwrap_err(),
        ConfigError::EmptyApiKey
    );
    assert_eq!(
        Endpoint::bedrock_runtime("us-east-1", Some("ftp://x"), &key("k"))
            .unwrap_err()
            .code(),
        "invalid_endpoint"
    );
    assert_eq!(ConfigError::EmptyRegion.code(), "empty_region");
    let debug = format!("{model:?}");
    assert!(!debug.contains("secret"));

    let response = model
        .generate(
            &Thread::new("system", "prompt"),
            &[serde_json::json!({"name": "broken"})],
        )
        .await;
    assert!(
        matches!(response.stop, StopReason::Error { ref code, .. } if code == "invalid_tool_definition")
    );
}

#[tokio::test]
async fn bedrock_requests_are_sigv4_signed_with_the_session_token() {
    let (endpoint, captured) = serve(sse_text()).await;
    let credentials = Credentials::new(
        "AKIDEXAMPLE",
        "secret",
        Some("session".to_owned()),
        None,
        "test",
    );
    let sigv4 = SigV4::with_provider("ca-central-1", SharedCredentialsProvider::new(credentials));
    let model = AnthropicModel::new(
        messages(&endpoint),
        "anthropic.claude-test",
        Auth::SigV4(sigv4),
        64,
    )
    .unwrap();
    let debug = format!("{model:?}");
    assert!(debug.contains("ca-central-1") && !debug.contains("secret"));

    let response = model.generate(&Thread::new("system", "hi"), &[]).await;

    assert_eq!(response.stop, StopReason::EndTurn);
    let head = captured.await.unwrap().head.to_ascii_lowercase();
    assert!(!head.contains("x-api-key"));
    assert!(head.contains("authorization: aws4-hmac-sha256 credential=akidexample/"));
    assert!(head.contains("/ca-central-1/bedrock-mantle/aws4_request"));
    assert!(head.contains(
        "signedheaders=anthropic-version;content-type;host;x-amz-date;x-amz-security-token"
    ));
    assert!(head.contains("x-amz-security-token: session"));
    assert!(head.contains("x-amz-date: "));
    assert_eq!(
        format!("{:?}", Endpoint::bedrock_mantle("ca-central-1").unwrap()),
        "Messages(https://bedrock-mantle.ca-central-1.api.aws/anthropic/v1/messages)"
    );
}

#[tokio::test]
async fn bedrock_runtime_invokes_the_model_with_a_bearer_token() {
    let (base, captured) = serve_frames(vec![chunk_frames(&sse_text())]).await;
    let endpoint =
        Endpoint::bedrock_runtime("us-east-1", Some(&base), &key("bedrock-key")).unwrap();
    let model =
        AnthropicModel::new(endpoint, "us.anthropic.claude-test", key("bedrock-key"), 64).unwrap();
    assert!(!format!("{model:?}").contains("bedrock-key"));

    let response = model
        .generate(&Thread::new("Be useful.", "héllo ☕"), &[])
        .await;

    assert_eq!(response.text, "bonjour ☕");
    assert_eq!(response.stop, StopReason::EndTurn);
    let request = captured.await.unwrap().remove(0);
    let head = request.head.to_ascii_lowercase();
    assert!(head.starts_with("post /model/us.anthropic.claude-test/invoke-with-response-stream "));
    assert!(head.contains("authorization: bearer bedrock-key"));
    assert!(!head.contains("x-api-key"));
    assert_eq!(request.body["anthropic_version"], "bedrock-2023-05-31");
    assert_eq!(request.body["max_tokens"], 64);
    assert_eq!(request.body["system"], "Be useful.");
    assert!(request.body.get("model").is_none());
    assert!(request.body.get("stream").is_none());
}

#[tokio::test]
async fn bedrock_runtime_signs_with_sigv4_and_types_stream_exceptions() {
    let (base, captured) =
        serve_frames(vec![exception_frame("throttlingException", "slow down")]).await;
    let credentials = Credentials::new("AKIDEXAMPLE", "secret", None, None, "test");
    let sigv4 = SigV4::with_provider("us-west-2", SharedCredentialsProvider::new(credentials));
    let endpoint =
        Endpoint::bedrock_runtime("us-west-2", Some(&base), &Auth::SigV4(sigv4.clone())).unwrap();
    let model =
        AnthropicModel::new(endpoint, "us.anthropic.claude-test", Auth::SigV4(sigv4), 64).unwrap();

    let response = model.generate(&Thread::new("system", "hi"), &[]).await;

    assert!(
        matches!(response.stop, StopReason::Error { ref code, ref message } if code == "rate_limited" && message == "slow down"),
        "{:?}",
        response.stop
    );
    let head = captured.await.unwrap().remove(0).head.to_ascii_lowercase();
    assert!(head.contains("authorization: aws4-hmac-sha256 credential=akidexample/"));
    assert!(head.contains("/us-west-2/bedrock/aws4_request"));
}

struct Echo;

impl Tool for Echo {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn definition(&self) -> Value {
        serde_json::json!({
            "name": "echo", "description": "Echo text",
            "input_schema": {"type": "object", "properties": {"text": {"type": "string"}}}
        })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<agent::ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move { Ok(request.args["text"].as_str().unwrap().to_owned().into()) })
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

struct Captured {
    head: String,
    body: Value,
}

struct Reply {
    status: u16,
    content_type: &'static str,
    body: String,
}

impl Reply {
    fn stream(body: String) -> Self {
        Self {
            status: 200,
            content_type: "text/event-stream",
            body,
        }
    }

    fn error(status: u16, kind: &str) -> Self {
        Self::failure(status, kind, "test failure")
    }

    fn failure(status: u16, kind: &str, message: &str) -> Self {
        Self {
            status,
            content_type: "application/json",
            body: serde_json::json!({
                "type": "error", "error": {"type": kind, "message": message}
            })
            .to_string(),
        }
    }
}

async fn serve(body: String) -> (String, tokio::task::JoinHandle<Captured>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let captured = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_request(&mut stream).await;
        write_response(&mut stream, &body).await;
        request
    });
    (format!("http://{address}/v1/messages"), captured)
}

async fn serve_many(bodies: Vec<String>) -> (String, tokio::task::JoinHandle<Vec<Captured>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let captured = tokio::spawn(async move {
        let mut requests = Vec::new();
        for body in bodies {
            let (mut stream, _) = listener.accept().await.unwrap();
            requests.push(read_request(&mut stream).await);
            write_response(&mut stream, &body).await;
        }
        requests
    });
    (format!("http://{address}/v1/messages"), captured)
}

async fn serve_replies(replies: Vec<Reply>) -> (String, tokio::task::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let count = replies.len();
    let task = tokio::spawn(async move {
        for reply in replies {
            let (mut stream, _) = listener.accept().await.unwrap();
            drop(read_request(&mut stream).await);
            write_reply(&mut stream, &reply).await;
        }
        count
    });
    (format!("http://{address}/v1/messages"), task)
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> Captured {
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
            return Captured {
                head: text[..start].to_owned(),
                body: serde_json::from_slice(&bytes[start..start + length]).unwrap(),
            };
        }
    }
}

fn content_length(line: &str) -> Option<usize> {
    line.to_ascii_lowercase()
        .strip_prefix("content-length: ")?
        .parse()
        .ok()
}

async fn write_response(stream: &mut tokio::net::TcpStream, body: &str) {
    let head = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body.as_bytes()).await.unwrap();
}

async fn write_reply(stream: &mut tokio::net::TcpStream, reply: &Reply) {
    let reason = if reply.status == 200 { "OK" } else { "Error" };
    let head = format!(
        "HTTP/1.1 {} {reason}\r\ncontent-type: {}\r\ncontent-length: {}\r\nretry-after: 0\r\nconnection: close\r\n\r\n",
        reply.status,
        reply.content_type,
        reply.body.len()
    );
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(reply.body.as_bytes()).await.unwrap();
}

fn sse_text() -> String {
    [
        r#"data: {"type":"message_start","message":{"content":[],"stop_reason":null}}\n\n"#,
        r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}\n\n"#,
        r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"bonjour "}}\n\n"#,
        r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"☕"}}\n\n"#,
        r#"data: {"type":"content_block_stop","index":0}\n\n"#,
        r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn"}}\n\n"#,
        r#"data: {"type":"message_stop"}\n\n"#,
    ]
    .concat()
    .replace("\\n", "\n")
}

fn sse_tool() -> String {
    [
        r#"data: {"type":"message_start","message":{"content":[],"stop_reason":null}}\n\n"#,
        r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}\n\n"#,
        r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"consider echo"}}\n\n"#,
        r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig-1"}}\n\n"#,
        r#"data: {"type":"content_block_stop","index":0}\n\n"#,
        r#"data: {"type":"content_block_start","index":1,"content_block":{"type":"redacted_thinking","data":"opaque-1"}}\n\n"#,
        r#"data: {"type":"content_block_stop","index":1}\n\n"#,
        r#"data: {"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"toolu_echo","name":"echo","input":{}}}\n\n"#,
        r#"data: {"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"text\":\"café "}}\n\n"#,
        r#"data: {"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"☕\"}"}}\n\n"#,
        r#"data: {"type":"content_block_stop","index":2}\n\n"#,
        r#"data: {"type":"message_delta","delta":{"stop_reason":"tool_use"}}\n\n"#,
        r#"data: {"type":"message_stop"}\n\n"#,
    ]
    .concat()
    .replace("\\n", "\n")
}

fn sse_answer() -> String {
    [
        r#"data: {"type":"message_start","message":{"content":[],"stop_reason":null}}\n\n"#,
        r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}\n\n"#,
        r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"café ☕"}}\n\n"#,
        r#"data: {"type":"content_block_stop","index":0}\n\n"#,
        r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn"}}\n\n"#,
        r#"data: {"type":"message_stop"}\n\n"#,
    ]
    .concat()
    .replace("\\n", "\n")
}

fn key(value: &str) -> Auth {
    Auth::ApiKey(value.to_owned())
}

fn messages(url: &str) -> Endpoint {
    Endpoint::messages(url).unwrap()
}

fn anthropic_api() -> Endpoint {
    messages("https://api.anthropic.com/v1/messages")
}

/// Event-stream frames wrapping each SSE data event as a Bedrock `chunk`.
fn chunk_frames(sse: &str) -> Vec<u8> {
    let mut frames = Vec::new();
    for data in sse
        .lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim_start))
    {
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let payload = serde_json::json!({"bytes": encoded}).to_string();
        let message = Frame::new(payload.into_bytes())
            .add_header(Header::new(
                ":message-type",
                HeaderValue::String("event".into()),
            ))
            .add_header(Header::new(
                ":event-type",
                HeaderValue::String("chunk".into()),
            ))
            .add_header(Header::new(
                ":content-type",
                HeaderValue::String("application/json".into()),
            ));
        write_message_to(&message, &mut frames).unwrap();
    }
    frames
}

fn exception_frame(exception: &str, text: &str) -> Vec<u8> {
    let payload = serde_json::json!({"message": text}).to_string();
    let message = Frame::new(payload.into_bytes())
        .add_header(Header::new(
            ":message-type",
            HeaderValue::String("exception".into()),
        ))
        .add_header(Header::new(
            ":exception-type",
            HeaderValue::String(exception.to_owned().into()),
        ))
        .add_header(Header::new(
            ":content-type",
            HeaderValue::String("application/json".into()),
        ));
    let mut frames = Vec::new();
    write_message_to(&message, &mut frames).unwrap();
    frames
}

/// Serves one Bedrock event-stream body per connection and returns the base URL.
async fn serve_frames(bodies: Vec<Vec<u8>>) -> (String, tokio::task::JoinHandle<Vec<Captured>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let captured = tokio::spawn(async move {
        let mut requests = Vec::new();
        for body in bodies {
            let (mut stream, _) = listener.accept().await.unwrap();
            requests.push(read_request(&mut stream).await);
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/vnd.amazon.eventstream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).await.unwrap();
            stream.write_all(&body).await.unwrap();
        }
        requests
    });
    (format!("http://{address}"), captured)
}

#[tokio::test]
async fn unsupported_output_schema_is_not_retried_as_plain_text() {
    let schema = serde_json::json!({"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":false});
    let profile = ExecutionProfile::default()
        .with_output_schema(agent::OutputSchema::new("answer", schema).unwrap());
    let thread = Thread::new_with_profile("system", "prompt", profile);
    let (endpoint, captured) = serve_replies(vec![Reply::failure(
        400,
        "invalid_request_error",
        "output schemas are unsupported",
    )])
    .await;
    let model = AnthropicModel::new(messages(&endpoint), "model", key("secret"), 1024).unwrap();
    let response = model.generate(&thread, &[]).await;
    assert!(
        matches!(response.stop, StopReason::Error { ref code, .. } if code == "invalid_request")
    );
    assert_eq!(captured.await.unwrap(), 1);
}
