//! Terminal validation must survive an exit after the assistant journal commit.
use std::{io::Write, process::Command, time::Duration};

use agent::{
    Event, ExecutionProfile, Message, Model, ModelResponse, Outcome, OutputSchema, StopReason,
    Thread,
};
use brainstone_runtime::{Address, OperationId, Prompt, Runtime, SqliteStore, Store};
use serde_json::{Value, json};

const LEASE: Duration = Duration::from_secs(30);
const CRASH: i32 = 73;

fn profile() -> ExecutionProfile {
    let schema = OutputSchema::new("answer", json!({"type":"object", "properties":{"value":{"type":"string"}}, "required":["value"], "additionalProperties":false})).unwrap();
    ExecutionProfile::default().with_output_schema(schema)
}

fn prompt() -> Prompt {
    Prompt::new(
        Address::new("test", "structured", "main").unwrap(),
        "one",
        "Return JSON.",
        "hello",
    )
    .unwrap()
    .with_profile(profile())
}

struct NoInference;
impl Model for NoInference {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "Recovery must settle the stored answer without inference."
    )]
    async fn generate(&self, _: &Thread, _: &[Value]) -> ModelResponse {
        panic!("a durable final response must not be generated again")
    }
}

#[tokio::test]
async fn resumed_answers_obey_the_schema_accepted_before_the_crash() {
    for (text, valid) in [("{\"value\":\"Zoë ☕\"}", true), ("{\"value\":42}", false)] {
        let directory = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", directory.path().join("state.db").display());
        let id = crash(&url, text);
        let store = SqliteStore::connect(&url).await.unwrap();
        let runtime = Runtime::new(store.clone());
        let operation = runtime
            .run_operation(id, &NoInference, &agent::Registry::default())
            .await
            .unwrap()
            .unwrap();
        assert_outcome(operation.outcome.unwrap(), valid);
        let duplicate = runtime.accept(&prompt()).await.unwrap();
        assert!(duplicate.duplicate);
        assert_outcome(duplicate.operation.outcome.unwrap(), valid);
        store.close().await;
    }
}

fn assert_outcome(outcome: Outcome, valid: bool) {
    match (outcome, valid) {
        (Outcome::Completed { text }, true) => assert_eq!(
            profile().output_schema().unwrap().validate(&text).unwrap()["value"],
            "Zoë ☕"
        ),
        (Outcome::Failed { code, .. }, false) => assert_eq!(code, "structured_output_invalid"),
        other => panic!("wrong restored outcome: {other:?}"),
    }
}

fn crash(url: &str, text: &str) -> OperationId {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "structured_crash_child",
            "--ignored",
            "--nocapture",
        ])
        .env("BRAINSTONE_STRUCTURED_DB", url)
        .env("BRAINSTONE_STRUCTURED_TEXT", text)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(CRASH),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("RECEIPT "))
        .unwrap();
    OperationId::new(id.parse().unwrap()).unwrap()
}

#[tokio::test]
#[ignore = "abrupt subprocess crash fixture"]
async fn structured_crash_child() {
    let store = SqliteStore::connect(&std::env::var("BRAINSTONE_STRUCTURED_DB").unwrap())
        .await
        .unwrap();
    let id = store.accept(&prompt()).await.unwrap().operation.id;
    let claim = store
        .claim("structured-child", LEASE)
        .await
        .unwrap()
        .unwrap();
    let response = response(std::env::var("BRAINSTONE_STRUCTURED_TEXT").unwrap());
    store
        .record(
            &claim.lease,
            &Event::Appended {
                message: Message::Assistant(response),
            },
        )
        .await
        .unwrap();
    // Expiry is controlled here; the abrupt exit still tests committed WAL recovery.
    store.heartbeat(&claim.lease, Duration::ZERO).await.unwrap();
    println!("RECEIPT {}", id.get());
    std::io::stdout().flush().unwrap();
    std::process::exit(CRASH);
}

fn response(text: String) -> ModelResponse {
    ModelResponse {
        reasoning: Vec::new(),
        text,
        tool_calls: Vec::new(),
        stop: StopReason::EndTurn,
        model: Some("fixture".to_owned()),
    }
}
