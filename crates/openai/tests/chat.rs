//! Table-driven protocol cases intentionally exceed the production function budget.
#![allow(clippy::too_many_lines)]
use agent::{Message, Model, ModelResponse, Reasoning, StopReason, Thread};
use brainstone_openai::ChatCompletionsModel;
use serde_json::{Value, json};
use std::fmt::Write as _;

#[path = "support/http.rs"]
mod http;
use http::{Events, Reply, ReplyBody, serve_many, serve_once, serve_reply};

#[tokio::test]
async fn malformed_tools_are_rejected_before_contacting_a_compatible_endpoint() {
    let model =
        ChatCompletionsModel::unauthenticated("http://127.0.0.1:1/v1/chat/completions", "local")
            .unwrap();
    let response = model
        .generate(
            &Thread::new("s", "hello"),
            &[serde_json::json!({"name":"missing-schema"})],
        )
        .await;
    assert!(
        matches!(response.stop, StopReason::Error { ref code, .. } if code == "invalid_tool_definition")
    );
}

#[tokio::test]
async fn hosted_and_local_targets_send_full_stateless_history_without_unrequested_parameters() {
    let reasoning = json!({"reasoning_details":[{"type":"reasoning.encrypted","data":"opaque","signature":"sig"}]});
    let answer = json!({"choices":[{"message":{"role":"assistant","content":"bonjour ☕","reasoning_details":reasoning["reasoning_details"]},"finish_reason":"stop"}]});
    let (endpoint, captured) = serve_once(answer).await;
    let model = ChatCompletionsModel::unauthenticated(&endpoint, "local").unwrap();
    let actual = model.generate(&Thread::new("system", "héllo"), &[]).await;
    assert_eq!(actual.text, "bonjour ☕");
    assert_eq!(
        actual.reasoning,
        vec![Reasoning::Opaque {
            provider: brainstone_openai::CHAT_COMPLETIONS_PROVIDER.to_owned(),
            data: reasoning.clone()
        }]
    );
    let captured = captured.await.unwrap();
    assert_eq!(captured.authorization, "");
    assert_eq!(
        captured.body,
        json!({"model":"local","messages":[{"role":"system","content":"system"},{"role":"user","content":"héllo"}]})
    );
    let (endpoint, captured) =
        serve_once(json!({"choices":[{"message":{"content":"done"},"finish_reason":"stop"}]}))
            .await;
    let model = ChatCompletionsModel::new(&endpoint, "router/model", "secret").unwrap();
    let mut thread = Thread::new("system", "héllo");
    thread.push(Message::Assistant(actual));
    thread.push(Message::User("again".to_owned()));
    model.generate(&thread, &[]).await;
    let captured = captured.await.unwrap();
    assert_eq!(captured.authorization, "Bearer secret");
    assert_eq!(
        captured.body["messages"][2]["reasoning_details"],
        reasoning["reasoning_details"]
    );
    assert_eq!(captured.body["messages"][3]["content"], "again");
}

#[tokio::test]
async fn tools_and_results_round_trip_with_opaque_ids_and_unicode_arguments() {
    let answer = json!({"choices":[{"message":{"content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"lookup","arguments":"{\"text\":\"☕\"}"}}]},"finish_reason":"tool_calls"}]});
    let (endpoint, captured) = serve_once(answer).await;
    let model = ChatCompletionsModel::new(&endpoint, "router/model", "secret")
        .unwrap()
        .with_max_output_tokens(1024);
    let tools = vec![
        json!({"name":"lookup","description":"Look up a record","input_schema":{"type":"object"}}),
    ];
    let mut thread = Thread::new("system", "go");
    let actual = model.generate(&thread, &tools).await;
    assert_eq!(actual.stop, StopReason::ToolUse);
    assert_eq!(actual.tool_calls[0].args, json!({"text":"☕"}));
    let captured = captured.await.unwrap();
    assert_eq!(captured.body["tools"][0]["function"]["name"], "lookup");
    assert_eq!(captured.body["max_tokens"], 1024);
    thread.push(Message::Assistant(actual));
    thread.push(Message::ToolResults(vec![agent::ToolResult {
        id: "call_1".to_owned(),
        name: "lookup".to_owned(),
        content: "found".to_owned(),
        is_error: false,
        images: vec![],
    }]));
    let (endpoint, captured) =
        serve_once(json!({"choices":[{"message":{"content":"done"},"finish_reason":"stop"}]}))
            .await;
    ChatCompletionsModel::unauthenticated(&endpoint, "local")
        .unwrap()
        .generate(&thread, &tools)
        .await;
    let body = captured.await.unwrap().body;
    assert_eq!(body["messages"][2]["tool_calls"][0]["id"], "call_1");
    assert_eq!(
        body["messages"][3],
        json!({"role":"tool","tool_call_id":"call_1","content":"found"})
    );
}

#[tokio::test]
async fn stream_reassembles_tool_and_reasoning_fragments_before_settlement() {
    let chunks = [
        json!({"content":"bon","reasoning_details":[{"index":0,"type":"reasoning.encrypted","data":"first","signature":"si"}],"tool_calls":[{"index":0,"id":"c","type":"function","function":{"name":"lookup","arguments":"{\"x\":"}}]}),
        json!({"content":"jour ☕","reasoning_details":[{"index":0,"data":"second","signature":"g"}],"tool_calls":[{"index":0,"function":{"arguments":"1}"}}]}),
    ];
    let mut body = String::new();
    for delta in chunks {
        write!(
            body,
            "data: {}\n\n",
            json!({"choices":[{"index":0,"delta":delta,"finish_reason":null}]})
        )
        .unwrap();
    }
    write!(
        body,
        "data: {}\n\ndata: [DONE]\n\n",
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]})
    )
    .unwrap();
    let (endpoint, _) = serve_reply(Reply::sse(body)).await;
    let model = ChatCompletionsModel::unauthenticated(&endpoint, "local").unwrap();
    let mut sink = Events::default();
    let actual = model
        .generate_stream(&Thread::new("s", "go"), &[], &mut sink)
        .await
        .unwrap();
    assert_eq!(actual.stop, StopReason::ToolUse);
    assert_eq!(actual.text, "bonjour ☕");
    assert_eq!(actual.tool_calls[0].args, json!({"x":1}));
    assert!(
        matches!(&actual.reasoning[0], Reasoning::Opaque { data, .. } if data["reasoning_details"][0]["data"] == "firstsecond" && data["reasoning_details"][0]["signature"] == "sig")
    );
    assert_eq!(
        sink.0,
        vec![agent::Event::ModelDelta {
            delta: "bonjour ☕".to_owned()
        }]
    );
}

#[tokio::test]
async fn repeated_terminal_usage_chunks_preserve_the_settled_response() {
    for (delta, finish, expected) in [
        (
            json!({"role":"assistant","content":""}),
            "stop",
            StopReason::EndTurn,
        ),
        (json!({}), "stop", StopReason::EndTurn),
        (
            json!({"content":"late"}),
            "stop",
            StopReason::Error {
                code: "invalid_response".into(),
                message: String::new(),
            },
        ),
        (
            json!({"tool_calls":[{"index":0}]}),
            "stop",
            StopReason::Error {
                code: "invalid_response".into(),
                message: String::new(),
            },
        ),
        (
            json!({}),
            "length",
            StopReason::Error {
                code: "invalid_response".into(),
                message: String::new(),
            },
        ),
    ] {
        let body = format!(
            "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            json!({"choices":[{"index":0,"delta":{"content":"OK"},"finish_reason":"stop"}]}),
            json!({"choices":[{"index":0,"delta":delta,"finish_reason":finish}],"usage":{"total_tokens":13}})
        );
        let (endpoint, _) = serve_reply(Reply::sse(body)).await;
        let response = ChatCompletionsModel::unauthenticated(&endpoint, "router")
            .unwrap()
            .generate_stream(&Thread::new("s", "go"), &[], &mut Events::default())
            .await
            .unwrap();
        match expected {
            StopReason::EndTurn => {
                assert_eq!(response.stop, expected);
                assert_eq!(response.text, "OK");
            }
            StopReason::Error { code, .. } => {
                assert!(
                    matches!(response.stop, StopReason::Error { code: actual, .. } if actual == code)
                );
                assert_eq!(response.text, "");
                assert_eq!(response.tool_calls, []);
            }
            _ => unreachable!(),
        }
    }
}

#[tokio::test]
async fn incomplete_or_invalid_streams_never_return_partial_tool_calls() {
    let partial = format!(
        "data: {}\n\n",
        json!({"choices":[{"index":0,"delta":{"content":"partial","tool_calls":[{"index":0,"id":"c","type":"function","function":{"name":"danger","arguments":"{"}}]},"finish_reason":null}]})
    );
    for (body, expected) in [
        (partial, "transport_error"),
        ("data: [DONE]\n\n".to_owned(), "invalid_response"),
        (
            "data: {\"error\":{\"code\":\"context_length_exceeded\"}}\n\n".to_owned(),
            "context_window_exceeded",
        ),
    ] {
        let (endpoint, _) = serve_reply(Reply::sse(body)).await;
        let model = ChatCompletionsModel::unauthenticated(&endpoint, "local").unwrap();
        let response = model
            .generate_stream(&Thread::new("s", "go"), &[], &mut Events::default())
            .await
            .unwrap();
        assert!(matches!(response.stop,StopReason::Error {ref code,..} if code == expected));
        assert_eq!(response.tool_calls, []);
        assert_eq!(response.text, "");
    }
}

#[tokio::test]
async fn terminal_failures_and_truncation_preserve_stable_codes() {
    let cases = [
        (
            json!({"error":{"code":429,"message":"busy"}}),
            "rate_limited",
        ),
        (
            json!({"choices":[{"message":{"audio":{"id":"unsupported"}},"finish_reason":"stop"}]}),
            "invalid_response",
        ),
        (json!({"choices":[]}), "invalid_response"),
        (
            json!({"choices":[{"message":{"content":"partial"},"finish_reason":null}]}),
            "invalid_response",
        ),
        (
            json!({"choices":[{"message":{"content":null,"refusal":"no"},"finish_reason":"stop"}]}),
            "refused",
        ),
        (
            json!({"choices":[{"message":{"content":""},"finish_reason":"content_filter"}]}),
            "content_filtered",
        ),
        (
            json!({"choices":[{"message":{"tool_calls":[{"id":"c","type":"function","function":{"name":"danger","arguments":"{"}}]},"finish_reason":"tool_calls"}]}),
            "invalid_response",
        ),
    ];
    for (answer, expected) in cases {
        let (endpoint, _) = serve_once(answer).await;
        let response = ChatCompletionsModel::unauthenticated(&endpoint, "local")
            .unwrap()
            .generate(&Thread::new("s", "go"), &[])
            .await;
        assert!(matches!(response.stop,StopReason::Error {ref code,..} if code == expected));
    }
    let (endpoint,_) = serve_once(json!({"choices":[{"message":{"tool_calls":[{"id":"c","type":"function","function":{"name":"danger","arguments":"{"}}]},"finish_reason":"length"}]})).await;
    let response = ChatCompletionsModel::unauthenticated(&endpoint, "local")
        .unwrap()
        .generate(&Thread::new("s", "go"), &[])
        .await;
    assert_eq!(response.stop, StopReason::MaxTokens);
    assert_eq!(response.tool_calls[0].args, Value::Null);
}

#[tokio::test]
async fn status_retries_share_the_responses_transport_policy() {
    let unavailable = Reply {
        status: 503,
        headers: vec![("retry-after", "0")],
        body: ReplyBody::Json(json!({"error":{"message":"busy"}})),
        content_type: "application/json",
    };
    let ok = Reply::ok(json!({"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}]}));
    let (endpoint, captured) = serve_many(vec![unavailable, ok]).await;
    let response = ChatCompletionsModel::unauthenticated(&endpoint, "local")
        .unwrap()
        .generate(&Thread::new("s", "go"), &[])
        .await;
    assert_eq!(response.text, "ok");
    let requests = captured.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].body, requests[1].body);
}

#[tokio::test]
async fn foreign_reasoning_fails_before_any_request() {
    let thread = Thread::restore(
        "s",
        vec![
            Message::User("go".to_owned()),
            Message::Assistant(ModelResponse {
                model: None,
                text: "previous".to_owned(),
                tool_calls: vec![],
                stop: StopReason::EndTurn,
                reasoning: vec![Reasoning::Opaque {
                    provider: brainstone_openai::OPENAI_RESPONSES_PROVIDER.to_owned(),
                    data: json!({"type":"reasoning"}),
                }],
            }),
        ],
    )
    .unwrap();
    let response = ChatCompletionsModel::unauthenticated("http://127.0.0.1:1/chat", "local")
        .unwrap()
        .generate(&thread, &[])
        .await;
    assert!(matches!(response.stop,StopReason::Error {ref code,..} if code == "foreign_reasoning"));
}

#[tokio::test]
async fn provider_parameters_extend_both_protocols_without_overriding_durable_state() {
    for key in [
        "model",
        "messages",
        "input",
        "instructions",
        "tools",
        "stream",
        "store",
        "previous_response_id",
        "conversation",
        "background",
        "n",
    ] {
        let parameters = serde_json::Map::from_iter([(key.to_owned(), json!(true))]);
        let error = brainstone_openai::RequestParameters::new(parameters)
            .err()
            .unwrap();
        assert_eq!(error.code(), "invalid_request_parameter");
        assert!(!error.to_string().contains("true"));
    }
    let parameters = brainstone_openai::RequestParameters::new(serde_json::Map::from_iter([
        ("max_completion_tokens".to_owned(), json!(2048)),
        ("provider".to_owned(), json!({"only":["chosen-upstream"]})),
    ]))
    .unwrap();
    let (endpoint, captured) =
        serve_once(json!({"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}]})).await;
    let model = ChatCompletionsModel::unauthenticated(&endpoint, "local")
        .unwrap()
        .with_parameters(parameters.clone());
    assert_eq!(
        model.generate(&Thread::new("s", "go"), &[]).await.text,
        "ok"
    );
    let body = captured.await.unwrap().body;
    assert_eq!(body["max_completion_tokens"], 2048);
    assert!(body.get("max_tokens").is_none());
    let (endpoint, captured) = serve_once(json!({"status":"completed","output":[]})).await;
    let model = brainstone_openai::ResponsesModel::unauthenticated(&endpoint, "local")
        .unwrap()
        .with_parameters(parameters);
    assert_eq!(
        model.generate(&Thread::new("s", "go"), &[]).await.stop,
        StopReason::EndTurn
    );
    assert_eq!(
        captured.await.unwrap().body["provider"],
        json!({"only":["chosen-upstream"]})
    );
    let parameters = brainstone_openai::RequestParameters::new(serde_json::Map::from_iter([(
        "max_tokens".to_owned(),
        json!(1),
    )]))
    .unwrap();
    let model = ChatCompletionsModel::unauthenticated("http://127.0.0.1:1/chat", "local")
        .unwrap()
        .with_max_output_tokens(2)
        .with_parameters(parameters);
    let response = model.generate(&Thread::new("s", "go"), &[]).await;
    assert!(matches!(response.stop,StopReason::Error {ref code,..} if code=="invalid_request"));
}

#[test]
fn debug_output_never_exposes_endpoint_query_credentials_or_header_values() {
    let endpoint = "http://localhost/chat?api_key=query-secret#private-fragment";
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "x-key",
        reqwest::header::HeaderValue::from_static("header-secret"),
    );
    let chat = ChatCompletionsModel::new(endpoint, "model", "bearer-secret")
        .unwrap()
        .with_headers(headers.clone());
    let responses = brainstone_openai::ResponsesModel::new(endpoint, "model", "bearer-secret")
        .unwrap()
        .with_headers(headers);
    for debug in [format!("{chat:?}"), format!("{responses:?}")] {
        assert!(debug.contains("http://localhost/chat"));
        for secret in [
            "query-secret",
            "private-fragment",
            "header-secret",
            "bearer-secret",
        ] {
            assert!(!debug.contains(secret));
        }
    }
}

#[tokio::test]
async fn unsupported_output_schemas_fail_without_a_plain_text_retry() {
    let schema = json!({"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":false});
    let profile = agent::ExecutionProfile::default()
        .with_output_schema(agent::OutputSchema::new("answer", schema.clone()).unwrap());
    let thread = Thread::new_with_profile("system", "prompt", profile);
    for responses in [false, true] {
        let reply = Reply {
            status: 400,
            headers: Vec::new(),
            content_type: "application/json",
            body: ReplyBody::Json(json!({"error":{"message":"output schemas are unsupported"}})),
        };
        let (endpoint, captured) = serve_reply(reply).await;
        let response = if responses {
            brainstone_openai::ResponsesModel::new(&endpoint, "model", "secret")
                .unwrap()
                .generate(&thread, &[])
                .await
        } else {
            ChatCompletionsModel::new(&endpoint, "model", "secret")
                .unwrap()
                .generate(&thread, &[])
                .await
        };
        assert!(
            matches!(response.stop, StopReason::Error { ref code, .. } if code == "invalid_request")
        );
        let body = captured.await.unwrap().body;
        let encoded = if responses {
            &body["text"]["format"]["schema"]
        } else {
            &body["response_format"]["json_schema"]["schema"]
        };
        assert_eq!(encoded, &schema);
    }
}

#[tokio::test]
async fn raw_output_parameters_remain_available_without_a_profile_schema() {
    for responses in [false, true] {
        let (name, value, reply) = if responses {
            (
                "text",
                json!({"verbosity":"low"}),
                json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"legacy"}]}]}),
            )
        } else {
            (
                "response_format",
                json!({"type":"json_object"}),
                json!({"choices":[{"message":{"content":"legacy"},"finish_reason":"stop"}]}),
            )
        };
        let parameters = brainstone_openai::RequestParameters::new(serde_json::Map::from_iter([(
            name.to_owned(),
            value.clone(),
        )]))
        .unwrap();
        let (endpoint, captured) = serve_once(reply).await;
        let thread = Thread::new("system", "prompt");
        let response = if responses {
            brainstone_openai::ResponsesModel::new(&endpoint, "model", "secret")
                .unwrap()
                .with_parameters(parameters)
                .generate(&thread, &[])
                .await
        } else {
            ChatCompletionsModel::new(&endpoint, "model", "secret")
                .unwrap()
                .with_parameters(parameters)
                .generate(&thread, &[])
                .await
        };
        assert_eq!(response.text, "legacy");
        assert_eq!(captured.await.unwrap().body[name], value);
    }
}

#[tokio::test]
async fn profile_owned_output_formats_reject_raw_overrides_before_http() {
    let schema = json!({"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":false});
    let profile = agent::ExecutionProfile::default()
        .with_output_schema(agent::OutputSchema::new("answer", schema).unwrap());
    let thread = Thread::new_with_profile("system", "prompt", profile);
    for responses in [false, true] {
        for streaming in [false, true] {
            let name = if responses { "text" } else { "response_format" };
            let parameters = brainstone_openai::RequestParameters::new(serde_json::Map::from_iter(
                [(name.to_owned(), json!({"type":"text"}))],
            ))
            .unwrap();
            // An attempted request to this closed port would be a transport error.
            let endpoint = "http://127.0.0.1:1/v1/responses";
            let response = if responses {
                let model = brainstone_openai::ResponsesModel::new(endpoint, "model", "secret")
                    .unwrap()
                    .with_parameters(parameters);
                if streaming {
                    model
                        .generate_stream(&thread, &[], &mut Events::default())
                        .await
                        .unwrap()
                } else {
                    model.generate(&thread, &[]).await
                }
            } else {
                let model = ChatCompletionsModel::new(endpoint, "model", "secret")
                    .unwrap()
                    .with_parameters(parameters);
                if streaming {
                    model
                        .generate_stream(&thread, &[], &mut Events::default())
                        .await
                        .unwrap()
                } else {
                    model.generate(&thread, &[]).await
                }
            };
            assert!(
                matches!(response.stop, StopReason::Error { ref code, .. } if code == "invalid_request")
            );
        }
    }
}
