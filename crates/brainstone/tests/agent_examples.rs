//! Example tools validate model input and return documented stable failures.
#[path = "support/model_endpoint.rs"]
mod http;
#[path = "../examples/support/order_lookup.rs"]
mod orders;

use brainstone::openai::ResponsesModel;
use brainstone::runtime::SqliteStore;
use brainstone::{Address, Host, Outcome, Prompt, Registry, Runtime, Tool, ToolRequest};
use serde_json::{Value, json};

#[tokio::test]
async fn order_lookup_reports_facts_and_stable_failure_codes() {
    let lookup = orders::tool().unwrap();
    let output = lookup
        .call(request(json!({"order_id":"BS-1001"})))
        .await
        .unwrap();
    let facts: Value = serde_json::from_str(&output.text).unwrap();
    assert_eq!(facts["value"]["status"], "shipped");
    assert_eq!(
        lookup
            .call(request(json!({"order_id":"unknown"})))
            .await
            .unwrap_err()
            .code,
        "order_not_found"
    );
}

#[tokio::test]
async fn order_lookup_rejects_malformed_model_arguments() {
    for args in [
        Value::Null,
        json!({}),
        json!({"order_id":42}),
        json!({"order_id":""}),
        json!({"order_id":"BS-1001", "extra":true}),
    ] {
        assert_eq!(
            orders::tool()
                .unwrap()
                .call(request(args))
                .await
                .unwrap_err()
                .code,
            "invalid_tool_arguments"
        );
    }
}

#[tokio::test]
async fn order_lookup_rejects_a_blank_business_identifier() {
    let error = orders::tool()
        .unwrap()
        .call(request(json!({"order_id":"  "})))
        .await
        .unwrap_err();
    assert_eq!(error.code, "invalid_arguments");
}

fn request(args: Value) -> ToolRequest {
    ToolRequest {
        args,
        idempotency_key: Some("test-call".to_owned()),
        context: None,
    }
}

#[tokio::test]
async fn chat_history_survives_reopening_and_retries_do_not_call_the_provider() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}", directory.path().join("chat.db").display());
    let first = prompt("request-1", "My name is Zoë.");
    let second = prompt("request-2", "What is my name?");
    turn(&url, &first, "Hello Zoë.").await;
    let captured = turn(&url, &second, "Zoë").await;
    assert!(captured.contains("My name is Zoë."));
    assert!(captured.contains("Hello Zoë."));
    retry(&url, &first).await;
}

async fn retry(url: &str, first: &Prompt) {
    let store = SqliteStore::connect(url).await.unwrap();
    let model = ResponsesModel::new("http://127.0.0.1:1/responses", "example", "test").unwrap();
    let host = Host::new(Runtime::new(store.clone()), model, Registry::default());
    let accepted = host.accept(first).await.unwrap();
    assert!(accepted.duplicate);
    assert_eq!(
        host.execute(accepted.operation.id).await.unwrap().outcome,
        Some(Outcome::Completed {
            text: "Hello Zoë.".to_owned()
        })
    );
    let changed = prompt("request-1", "Different input");
    assert_eq!(
        host.accept(&changed).await.unwrap_err().code(),
        "idempotency_conflict"
    );
    host.shutdown().await;
    store.close().await;
}

fn prompt(id: &str, text: &str) -> Prompt {
    Prompt::new(
        Address::new("chat", "local", "main").unwrap(),
        id,
        "Be useful.",
        text,
    )
    .unwrap()
}

async fn turn(url: &str, prompt: &Prompt, answer: &str) -> String {
    let response = json!({"status":"completed", "output":[{"type":"message", "content":[{"type":"output_text", "text":answer}]}]});
    let event = json!({"type":"response.completed", "response":response});
    let (endpoint, captured) =
        http::serve_once(format!("data: {event}\n\n"), "text/event-stream").await;
    let model = ResponsesModel::new(&endpoint, "example", "test").unwrap();
    let store = SqliteStore::connect(url).await.unwrap();
    let host = Host::new(Runtime::new(store.clone()), model, Registry::default());
    let accepted = host.accept(prompt).await.unwrap();
    let outcome = host.execute(accepted.operation.id).await.unwrap().outcome;
    assert_eq!(
        outcome,
        Some(Outcome::Completed {
            text: answer.to_owned()
        })
    );
    host.shutdown().await;
    store.close().await;
    captured.await.unwrap()
}
