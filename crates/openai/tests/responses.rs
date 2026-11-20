//! Public HTTP contract tests are intentionally table-heavy and exempt from
//! the production function budget.
#![allow(clippy::too_many_lines)]

use agent::{
    Event, Image, ImageMediaType, Message, Model, ModelResponse, Reasoning, StopReason, Thread,
    ToolCall, ToolResult, UserImages,
};
use aws_credential_types::Credentials;
use aws_credential_types::provider::SharedCredentialsProvider;
use brainstone_openai::{
    Auth, ConfigError, Effort, OPENAI_RESPONSES_PROVIDER, ResponsesModel, SigV4,
};
use serde_json::{Value, json};
use tokio::net::TcpListener;

#[test]
fn configuration_is_validated_and_credentials_are_redacted() {
    let cases = [
        (("relative", "model", "secret"), "invalid_endpoint"),
        (
            ("file:///tmp/responses", "model", "secret"),
            "invalid_endpoint",
        ),
        (
            ("https://user:pass@api.example/responses", "model", "secret"),
            "invalid_endpoint",
        ),
        (
            ("https://api.example/responses", " ", "secret"),
            "empty_model",
        ),
        (
            ("https://api.example/responses", "model", " "),
            "empty_bearer",
        ),
    ];
    for ((endpoint, model, bearer), expected) in cases {
        let error = ResponsesModel::new(endpoint, model, bearer).unwrap_err();
        assert_eq!(error.code(), expected);
    }
    let model =
        ResponsesModel::new("https://api.example/responses", "model", "top-secret").unwrap();
    let debug = format!("{model:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("top-secret"));
    assert_eq!(
        ConfigError::MissingEnvironment {
            name: "OPENAI_API_KEY",
            message: "absent".to_owned()
        }
        .code(),
        "missing_environment"
    );
    assert_eq!(
        ConfigError::ClientBuild {
            message: "client".to_owned()
        }
        .code(),
        "client_build_failed"
    );
}

#[tokio::test]
async fn images_become_ordered_responses_content() {
    let response = json!({"status":"completed","output":[]});
    let (endpoint, request) = serve_once(response).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();
    let image = Image::new(ImageMediaType::Png, vec![1, 2, 3], 2, 3).unwrap();
    let second = Image::new(ImageMediaType::Jpeg, vec![4, 5], 4, 5).unwrap();
    let thread = Thread::restore(
        "system",
        vec![Message::UserImages(
            UserImages::new("inspect", vec![image, second]).unwrap(),
        )],
    )
    .unwrap();
    model.generate(&thread, &[]).await;
    let body = request.await.unwrap().body;
    let content = &body["input"][0]["content"];
    assert_eq!(content[0]["type"], "input_image");
    assert_eq!(content[0]["image_url"], "data:image/png;base64,AQID");
    assert_eq!(content[1]["image_url"], "data:image/jpeg;base64,BAU=");
    assert_eq!(content[2], json!({"type":"input_text","text":"inspect"}));
}

#[tokio::test]
async fn tool_result_attachments_become_function_output_content() {
    let response = json!({"status":"completed","output":[]});
    let (endpoint, request) = serve_once(response).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();
    let image = Image::new(ImageMediaType::Png, vec![9], 1, 1).unwrap();
    let thread = Thread::restore(
        "system",
        vec![
            Message::User("read it".to_owned()),
            Message::Assistant(ModelResponse {
                model: None,
                reasoning: Vec::new(),
                text: String::new(),
                tool_calls: vec![ToolCall {
                    id: "call_1".to_owned(),
                    name: "ReadThreadFile".to_owned(),
                    args: json!({}),
                }],
                stop: StopReason::ToolUse,
            }),
            Message::ToolResults(vec![ToolResult {
                id: "call_1".to_owned(),
                name: "ReadThreadFile".to_owned(),
                content: "found".to_owned(),
                is_error: false,
                images: vec![image],
            }]),
        ],
    )
    .unwrap();
    model.generate(&thread, &[]).await;
    let body = request.await.unwrap().body;
    let output = &body["input"][2]["output"];
    assert_eq!(body["input"][2]["type"], "function_call_output");
    assert_eq!(output[0]["image_url"], "data:image/png;base64,CQ==");
    assert_eq!(output[1], json!({"type":"input_text","text":"found"}));
}

#[tokio::test]
async fn bedrock_runtime_requests_are_sigv4_signed_for_the_bedrock_service() {
    let response = json!({
        "status": "completed",
        "output": [{"type": "message", "role": "assistant",
                    "content": [{"type": "output_text", "text": "ok"}]}]
    });
    let (endpoint, request) = serve_once(response).await;
    let base = endpoint.trim_end_matches("/v1/responses");
    let credentials = Credentials::for_tests_with_session_token();
    let sigv4 = SigV4::with_provider("us-west-2", SharedCredentialsProvider::new(credentials));
    let auth = Auth::SigV4(sigv4);
    let model =
        ResponsesModel::bedrock_runtime("us-west-2", Some(base), "us.openai.test", auth).unwrap();

    let actual = model.generate(&Thread::new("s", "hi"), &[]).await;

    assert_eq!(actual.text, "ok");
    let authorization = request.await.unwrap().authorization;
    assert!(
        authorization.starts_with("AWS4-HMAC-SHA256 "),
        "{authorization}"
    );
    assert!(
        authorization.contains("/us-west-2/bedrock/aws4_request"),
        "{authorization}"
    );
}

#[test]
fn bedrock_runtime_follows_the_documented_url() {
    let auth = || Auth::Bearer("bedrock-key".to_owned());
    let runtime =
        ResponsesModel::bedrock_runtime("ca-central-1", None, "us.openai.test", auth()).unwrap();

    let runtime = format!("{runtime:?}");
    assert!(
        runtime.contains("https://bedrock-runtime.ca-central-1.amazonaws.com/openai/v1/responses"),
        "{runtime}"
    );
    assert!(!runtime.contains("bedrock-key"));
    let error = ResponsesModel::bedrock_runtime(" ", None, "model", auth()).unwrap_err();
    assert_eq!(error.code(), "empty_region");
}

#[tokio::test]
async fn a_prompt_round_trips_through_the_responses_api() {
    let response = json!({
        "status": "completed",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "bonjour ☕"}]
        }]
    });
    let (endpoint, request) = serve_once(response).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret")
        .unwrap()
        .with_effort(Effort::XHigh)
        .with_max_output_tokens(4096);

    let thread = Thread::restore(
        "Be useful.",
        vec![
            Message::Summary("The user prefers French.".to_owned()),
            Message::User("héllo ☕".to_owned()),
        ],
    )
    .unwrap();
    let actual = model.generate(&thread, &[]).await;

    assert_eq!(
        actual,
        ModelResponse {
            model: None,
            reasoning: Vec::new(),
            text: "bonjour ☕".to_owned(),
            tool_calls: Vec::new(),
            stop: StopReason::EndTurn,
        }
    );
    assert_eq!(
        request.await.unwrap().body,
        json!({
            "model": "test-model",
            "instructions": "Be useful.",
            "input": [
                {"role": "user", "content": "Summary of earlier conversation:\nThe user prefers French."},
                {"role": "user", "content": "héllo ☕"}
            ],
            "tools": [],
            "store": false,
            "include": ["reasoning.encrypted_content"],
            "reasoning": {"effort": "xhigh"},
            "max_output_tokens": 4096
        })
    );
}

#[tokio::test]
async fn execution_profile_overrides_parent_model_and_effort() {
    let response = json!({"status":"completed","output":[]});
    let (endpoint, request) = serve_once(response).await;
    let model = ResponsesModel::new(&endpoint, "parent-model", "secret")
        .unwrap()
        .with_effort(Effort::High);
    let profile =
        agent::ExecutionProfile::new(Some("child-model"), Some(agent::Effort::Low), 7, 1).unwrap();
    let thread = Thread::new_with_profile("system", "task", profile);

    model.generate(&thread, &[]).await;

    let body = request.await.unwrap().body;
    assert_eq!(body["model"], "child-model");
    assert_eq!(body["reasoning"]["effort"], "low");
}

#[tokio::test]
async fn response_text_streams_before_the_settled_response() {
    let completed = json!({
        "status": "completed",
        "output": [{
            "type": "message",
            "content": [{"type": "output_text", "text": "bonjour ☕"}]
        }]
    });
    let body = format!(
        "data: {{\"type\":\"response.output_text.delta\",\"delta\":\"bon\"}}\n\n\
         data: {{\"type\":\"response.output_text.delta\",\"delta\":\"jour ☕\"}}\n\n\
         data: {}\n\n",
        json!({"type": "response.completed", "response": completed})
    );
    let (endpoint, request) = serve_reply(Reply::sse(body)).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();
    let mut sink = Events::default();

    let actual = model
        .generate_stream(&Thread::new("s", "hello"), &[], &mut sink)
        .await
        .expect("the sink accepts events");

    assert_eq!(actual.text, "bonjour ☕");
    assert_eq!(
        sink.0,
        vec![Event::ModelDelta {
            delta: "bonjour ☕".to_owned()
        }]
    );
    let body = request.await.unwrap().body;
    assert_eq!(body["stream"], true);
    assert!(body.get("reasoning").is_none());
}

#[tokio::test]
async fn opted_in_reasoning_summaries_stream_before_text_and_settle_opaquely() {
    let item = json!({
        "type": "reasoning", "id": "rs_1", "encrypted_content": "opaque",
        "summary": [{"type": "summary_text", "text": "pensé ☕"}]
    });
    let completed = json!({
        "status": "completed",
        "output": [item, {
            "type": "message",
            "content": [{"type": "output_text", "text": "oui"}]
        }]
    });
    let body = format!(
        "data: {{\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"pensé\"}}\n\n\
         data: {{\"type\":\"response.reasoning_text.delta\",\"delta\":\" ☕\"}}\n\n\
         data: {{\"type\":\"response.output_text.delta\",\"delta\":\"oui\"}}\n\n\
         data: {}\n\n",
        json!({"type": "response.completed", "response": completed})
    );
    let (endpoint, request) = serve_reply(Reply::sse(body)).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret")
        .unwrap()
        .with_effort(Effort::High)
        .with_reasoning_summary();
    let mut sink = Events::default();

    let actual = model
        .generate_stream(&Thread::new("s", "hello"), &[], &mut sink)
        .await
        .expect("the sink accepts events");

    assert_eq!(
        sink.0,
        vec![
            Event::ReasoningDelta {
                delta: "pensé ☕".to_owned()
            },
            Event::ModelDelta {
                delta: "oui".to_owned()
            },
        ]
    );
    assert_eq!(
        actual.reasoning,
        vec![Reasoning::Opaque {
            provider: OPENAI_RESPONSES_PROVIDER.to_owned(),
            data: item
        }]
    );
    let body = request.await.unwrap().body;
    assert_eq!(
        body["reasoning"],
        json!({"effort": "high", "summary": "auto"})
    );
}

#[tokio::test]
async fn reasoning_summaries_are_not_requested_unless_opted_in() {
    let response = json!({"status": "completed", "output": []});
    let (endpoint, request) = serve_once(response).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret")
        .unwrap()
        .with_effort(Effort::High);

    model.generate(&Thread::new("s", "hello"), &[]).await;

    let body = request.await.unwrap().body;
    assert_eq!(body["reasoning"], json!({"effort": "high"}));
}

#[tokio::test]
async fn complete_tool_history_is_replayed_without_losing_reasoning() {
    let response = json!({
        "status": "completed",
        "output": [{
            "type": "message",
            "content": [{"type": "output_text", "text": "done"}]
        }]
    });
    let (endpoint, request) = serve_once(response).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();
    let reasoning = json!({
        "type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "opaque"
    });
    let thread = Thread::restore(
        "Be useful.",
        vec![
            Message::User("find café ☕".to_owned()),
            Message::Assistant(ModelResponse {
                model: None,
                reasoning: vec![Reasoning::Opaque {
                    provider: OPENAI_RESPONSES_PROVIDER.to_owned(),
                    data: reasoning.clone(),
                }],
                text: "I will look.".to_owned(),
                tool_calls: vec![ToolCall {
                    id: "call_1".to_owned(),
                    name: "lookup".to_owned(),
                    args: json!({"q": "café ☕"}),
                }],
                stop: StopReason::ToolUse,
            }),
            Message::ToolResults(vec![ToolResult {
                id: "call_1".to_owned(),
                name: "lookup".to_owned(),
                content: "found".to_owned(),
                is_error: false,
                images: Vec::new(),
            }]),
        ],
    )
    .unwrap();
    let tools = [json!({
        "name": "lookup",
        "description": "Looks things up",
        "input_schema": {"type": "object", "properties": {"q": {"type": "string"}}}
    })];

    assert_eq!(model.generate(&thread, &tools).await.text, "done");
    assert_eq!(
        request.await.unwrap().body,
        json!({
            "model": "test-model",
            "instructions": "Be useful.",
            "input": [
                {"role": "user", "content": "find café ☕"},
                reasoning,
                {"role": "assistant", "content": "I will look."},
                {
                    "type": "function_call", "call_id": "call_1", "name": "lookup",
                    "arguments": "{\"q\":\"café ☕\"}"
                },
                {"type": "function_call_output", "call_id": "call_1", "output": "found"}
            ],
            "tools": [{
                "type": "function", "name": "lookup", "description": "Looks things up",
                "parameters": {"type": "object", "properties": {"q": {"type": "string"}}}
            }],
            "store": false
        })
    );
}

#[tokio::test]
async fn reasoning_text_and_function_calls_are_preserved_from_one_response() {
    let reasoning = json!({
        "type": "reasoning", "id": "rs_2", "summary": [], "encrypted_content": "ciphertext"
    });
    let response = json!({
        "status": "completed",
        "output": [
            reasoning.clone(),
            {"type": "message", "content": [
                {"type": "output_text", "text": "Let me "},
                {"type": "output_text", "text": "check."}
            ]},
            {
                "type": "function_call", "call_id": "call_2", "name": "lookup",
                "arguments": "{\"q\":\"東京\"}"
            }
        ]
    });
    let (endpoint, _) = serve_once(response).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();

    let actual = model.generate(&Thread::new("s", "search"), &[]).await;

    assert_eq!(
        actual,
        ModelResponse {
            model: None,
            reasoning: vec![Reasoning::Opaque {
                provider: OPENAI_RESPONSES_PROVIDER.to_owned(),
                data: reasoning,
            }],
            text: "Let me check.".to_owned(),
            tool_calls: vec![ToolCall {
                id: "call_2".to_owned(),
                name: "lookup".to_owned(),
                args: json!({"q": "東京"}),
            }],
            stop: StopReason::ToolUse,
        }
    );
}

#[tokio::test]
async fn output_limit_marks_even_parseable_tool_calls_as_truncated() {
    let response = json!({
        "status": "incomplete",
        "incomplete_details": {"reason": "max_output_tokens"},
        "output": [{
            "type": "function_call", "call_id": "call_cut", "name": "send",
            "arguments": "{\"confirmed\":true}"
        }]
    });
    let (endpoint, _) = serve_once(response).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();

    let actual = model.generate(&Thread::new("s", "go"), &[]).await;

    assert_eq!(actual.stop, StopReason::MaxTokens);
    assert_eq!(actual.tool_calls[0].id, "call_cut");
}

#[tokio::test]
async fn non_success_response_states_have_stable_failure_codes() {
    let cases = [
        (
            json!({
                "status": "incomplete", "incomplete_details": {"reason": "content_filter"},
                "output": []
            }),
            "content_filtered",
        ),
        (
            json!({
                "status": "failed", "error": {"code": "server_error", "message": "broke"},
                "output": []
            }),
            "provider_failed",
        ),
        (
            json!({
                "status": "incomplete", "incomplete_details": {"reason": "new_reason"},
                "output": []
            }),
            "invalid_response",
        ),
        (
            json!({"status": "cancelled", "output": []}),
            "request_cancelled",
        ),
        (
            json!({"status": "queued", "output": []}),
            "invalid_response",
        ),
    ];
    for (response, expected) in cases {
        let (endpoint, _) = serve_once(response).await;
        let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();
        let actual = model.generate(&Thread::new("s", "go"), &[]).await;
        let StopReason::Error { code, .. } = actual.stop else {
            panic!("expected an error response");
        };
        assert_eq!(code, expected);
    }
}

#[tokio::test]
async fn rate_limits_are_retried_with_the_same_authenticated_request() {
    let replies = vec![
        Reply {
            status: 429,
            headers: vec![("retry-after-ms", "0")],
            body: ReplyBody::Json(json!({"error": {"message": "slow down"}})),
            content_type: "application/json",
        },
        Reply::ok(json!({
            "status": "completed",
            "output": [{"type": "message", "content": [
                {"type": "output_text", "text": "recovered"}
            ]}]
        })),
    ];
    let (endpoint, requests) = serve_many(replies).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();

    let actual = model.generate(&Thread::new("s", "go"), &[]).await;

    assert_eq!(actual.text, "recovered");
    let requests = requests.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|request| request.authorization == "Bearer secret")
    );
    assert_eq!(requests[0].body, requests[1].body);
}

#[tokio::test]
async fn terminal_http_statuses_have_stable_codes_and_bounded_retries() {
    let cases = [
        (400, 1, "invalid_request"),
        (401, 1, "authentication_failed"),
        (403, 1, "permission_denied"),
        (404, 1, "model_or_endpoint_not_found"),
        (408, 4, "request_timeout"),
        (409, 4, "request_conflict"),
        (429, 4, "rate_limited"),
        (502, 4, "provider_unavailable"),
        (418, 1, "http_error"),
    ];
    for (status, attempts, expected) in cases {
        let reply = Reply {
            status,
            headers: vec![("retry-after", "0")],
            body: ReplyBody::Json(json!({"error": {"message": "wire detail"}})),
            content_type: "application/json",
        };
        let (endpoint, requests) = serve_many(vec![reply; attempts]).await;
        let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();
        let actual = model.generate(&Thread::new("s", "go"), &[]).await;
        let StopReason::Error { code, message } = actual.stop else {
            panic!("expected an error response");
        };
        assert_eq!(code, expected);
        assert_eq!(message, "wire detail");
        assert_eq!(requests.await.unwrap().len(), attempts);
    }
}

fn stop_code(response: ModelResponse) -> String {
    match response.stop {
        StopReason::Error { code, .. } => code,
        stop => panic!("expected an error stop, got {stop:?}"),
    }
}

/// The one rejection a caller can repair, by compacting, keeps its own code on
/// every path the provider reports it by: the HTTP body, a failed response, and a
/// stream error event.
#[tokio::test]
async fn an_overlong_input_is_typed_apart_from_other_failures() {
    let overlong = json!({"code": "context_length_exceeded", "message": "too many tokens"});
    let thread = Thread::new("s", "go");
    let (endpoint, _) = serve_reply(Reply {
        status: 400,
        headers: Vec::new(),
        body: ReplyBody::Json(json!({ "error": overlong })),
        content_type: "application/json",
    })
    .await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();
    assert_eq!(
        stop_code(model.generate(&thread, &[]).await),
        "context_window_exceeded"
    );

    let failed = json!({"status": "failed", "error": overlong, "output": []});
    let (endpoint, _) = serve_once(failed).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();
    assert_eq!(
        stop_code(model.generate(&thread, &[]).await),
        "context_window_exceeded"
    );

    let mut event = overlong.clone();
    event["type"] = json!("error");
    let (endpoint, _) = serve_reply(Reply::sse(format!("data: {event}\n\n"))).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();
    let streamed = model
        .generate_stream(&thread, &[], &mut Events::default())
        .await
        .expect("the sink accepts events");
    assert_eq!(stop_code(streamed), "context_window_exceeded");
}

#[tokio::test]
async fn invalid_tool_definitions_fail_before_network_io() {
    let definitions = [
        json!(null),
        json!({"description": "missing name", "input_schema": {}}),
        json!({"name": 7, "description": "wrong type", "input_schema": {}}),
        json!({"name": "", "description": "empty", "input_schema": {}}),
        json!({"name": "x", "description": 7, "input_schema": {}}),
        json!({"name": "x", "description": "bad schema", "input_schema": []}),
    ];
    let model = ResponsesModel::new("http://127.0.0.1:1", "test-model", "secret").unwrap();
    for definition in definitions {
        let actual = model.generate(&Thread::new("s", "go"), &[definition]).await;
        let StopReason::Error { code, .. } = actual.stop else {
            panic!("expected an error response");
        };
        assert_eq!(code, "invalid_tool_definition");
    }
}

#[tokio::test]
async fn foreign_or_malformed_reasoning_is_never_silently_dropped() {
    let reasoning = [
        Reasoning::Thinking {
            text: "thought".to_owned(),
            signature: Some("sig".to_owned()),
        },
        Reasoning::Redacted {
            data: "cipher".to_owned(),
        },
        Reasoning::Opaque {
            provider: "another-provider".to_owned(),
            data: json!({"type": "reasoning"}),
        },
        Reasoning::Opaque {
            provider: OPENAI_RESPONSES_PROVIDER.to_owned(),
            data: json!({"type": "not_reasoning"}),
        },
    ];
    let model = ResponsesModel::new("http://127.0.0.1:1", "test-model", "secret").unwrap();
    for item in reasoning {
        let thread = Thread::restore(
            "s",
            vec![
                Message::User("go".to_owned()),
                Message::Assistant(ModelResponse {
                    model: None,
                    reasoning: vec![item],
                    text: String::new(),
                    tool_calls: Vec::new(),
                    stop: StopReason::EndTurn,
                }),
                Message::User("again".to_owned()),
            ],
        )
        .unwrap();
        let actual = model.generate(&thread, &[]).await;
        let StopReason::Error { code, .. } = actual.stop else {
            panic!("expected an error response");
        };
        assert_eq!(code, "foreign_reasoning");
    }
}

#[tokio::test]
async fn completed_protocol_failures_are_typed() {
    let cases = [
        (
            json!({"status": "completed", "output": [{
                "type": "message", "content": [{"type": "refusal", "refusal": "no"}]
            }]}),
            "refused",
        ),
        (
            json!({"status": "completed", "output": [{"type": "future_item"}]}),
            "invalid_response",
        ),
        (
            json!({"status": "completed", "output": [{
                "type": "function_call", "call_id": "c", "name": "x", "arguments": "{"
            }]}),
            "invalid_response",
        ),
        (json!({"output": []}), "invalid_response"),
    ];
    for (response, expected) in cases {
        let (endpoint, _) = serve_once(response).await;
        let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();
        let actual = model.generate(&Thread::new("s", "go"), &[]).await;
        let StopReason::Error { code, .. } = actual.stop else {
            panic!("expected an error response");
        };
        assert_eq!(code, expected);
    }
}

#[tokio::test]
async fn malformed_arguments_in_a_truncated_call_are_safe_to_refuse() {
    let response = json!({
        "status": "incomplete",
        "incomplete_details": {"reason": "max_output_tokens"},
        "output": [{
            "type": "function_call", "call_id": "cut", "name": "danger", "arguments": "{"
        }]
    });
    let (endpoint, _) = serve_once(response).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();

    let actual = model.generate(&Thread::new("s", "go"), &[]).await;

    assert_eq!(actual.stop, StopReason::MaxTokens);
    assert_eq!(actual.tool_calls[0].args, Value::Null);
}

#[tokio::test]
async fn a_stream_cut_before_the_terminal_event_is_a_transport_error() {
    let body =
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"bon\"}\n\n: keepalive\n\n"
            .to_owned();
    let (endpoint, _) = serve_reply(Reply::sse(body)).await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();
    let mut sink = Events::default();

    let actual = model
        .generate_stream(&Thread::new("s", "hello"), &[], &mut sink)
        .await
        .expect("the sink accepts events");

    assert!(
        matches!(
            actual.stop,
            StopReason::Error { ref code, .. } if code == "transport_error"
        ),
        "{:?}",
        actual.stop
    );
}

#[tokio::test]
async fn malformed_success_json_is_an_invalid_response_not_a_transport_error() {
    let (endpoint, _) = serve_reply(Reply {
        status: 200,
        headers: Vec::new(),
        body: ReplyBody::Raw("not json"),
        content_type: "application/json",
    })
    .await;
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();

    let actual = model.generate(&Thread::new("s", "go"), &[]).await;

    let StopReason::Error { code, .. } = actual.stop else {
        panic!("expected an error response");
    };
    assert_eq!(code, "invalid_response");
}

#[tokio::test]
async fn connection_failures_are_transport_errors() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1/responses", listener.local_addr().unwrap());
    drop(listener);
    let model = ResponsesModel::new(&endpoint, "test-model", "secret").unwrap();

    let actual = model.generate(&Thread::new("s", "go"), &[]).await;

    let StopReason::Error { code, .. } = actual.stop else {
        panic!("expected an error response");
    };
    assert_eq!(code, "transport_error");
}

#[path = "support/http.rs"]
mod http;
use http::*;
