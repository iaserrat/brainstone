//! Required effects come from committed current-operation results across process exits.
#![allow(clippy::too_many_lines)] // Table-driven crash-boundary cases.
use agent::{
    Event, ExecutionProfile, Message, Model, ModelResponse, Outcome, Registry, StopReason, Thread,
    Tool, ToolContract, ToolError, ToolOutput, ToolRequest, ToolResult,
};
use brainstone_runtime::{Address, DurableStore, OperationId, Prompt, Runtime, Store};
use serde_json::Value;
use std::{io::Write, pin::Pin, process::Command, sync::Arc, time::Duration};

#[path = "support/database.rs"]
mod database;
use database::Database;

const LEASE: Duration = Duration::from_secs(30);
const CRASH: i32 = 73;

fn prompt(request: &str) -> Prompt {
    let contract = ToolContract::only(["Refund"])
        .unwrap()
        .require_success(["Refund"])
        .unwrap();
    Prompt::new(
        Address::new("test", "contract", "main").unwrap(),
        request,
        "system",
        "refund",
    )
    .unwrap()
    .with_profile(ExecutionProfile::default().with_tool_contract(contract))
}
fn response() -> ModelResponse {
    ModelResponse {
        reasoning: vec![],
        text: "done".into(),
        tool_calls: vec![],
        stop: StopReason::EndTurn,
        model: None,
    }
}
fn result() -> ToolResult {
    ToolResult {
        id: "one".into(),
        name: "Refund".into(),
        content: "{}".into(),
        is_error: false,
        images: vec![],
    }
}
struct NoInference;
impl Model for NoInference {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "Recovery must settle without inference."
    )]
    async fn generate(&self, _: &Thread, _: &[Value]) -> ModelResponse {
        panic!("stored final answer must not be generated again")
    }
}

#[tokio::test]
async fn crash_recovery_counts_only_committed_current_operation_successes() {
    for (case, expected) in cases() {
        recover_case(Database::sqlite(), case, expected).await;
    }
}

#[tokio::test]
#[ignore = "requires a dedicated PostgreSQL database"]
async fn postgres_tool_contract_recovers_committed_operation_evidence() {
    for (case, expected) in cases() {
        recover_case(Database::postgres().await, case, expected).await;
    }
}

fn cases() -> [(&'static str, Option<&'static str>); 3] {
    [
        ("committed", None),
        ("progress", Some("tool_requirement_unmet")),
        ("prior", Some("tool_requirement_unmet")),
    ]
}

async fn recover_case(database: Database, case: &str, expected: Option<&str>) {
    let id = crash(&database.url, case);
    let store = DurableStore::connect(&database.url).await.unwrap();
    let operation = Runtime::new(store.clone())
        .run_operation(id, &NoInference, &tools())
        .await
        .unwrap()
        .unwrap();
    match (operation.outcome.unwrap(), expected) {
        (Outcome::Completed { .. }, None) => {}
        (Outcome::Failed { code, .. }, Some(expected)) => assert_eq!(code, expected),
        other => panic!("wrong recovered outcome for {case}: {other:?}"),
    }
    store.close().await;
    database.close().await;
}

#[tokio::test]
async fn legacy_compaction_does_not_decode_invalidated_event_bodies() {
    let database = Database::sqlite();
    let id = crash(&database.url, "legacy");
    let pool = sqlx::SqlitePool::connect(&database.url).await.unwrap();
    sqlx::query(
        "UPDATE operation_events SET schema_version=99 WHERE operation_id=?1 AND sequence=0",
    )
    .bind(id.get())
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
    let store = DurableStore::connect(&database.url).await.unwrap();
    let operation = Runtime::new(store.clone())
        .run_operation(id, &NoInference, &tools())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(operation.outcome, Some(Outcome::Completed { .. })));
    store.close().await;
    database.close().await;
}

#[tokio::test]
async fn unknown_records_before_compaction_are_rejected() {
    for update in [
        "UPDATE operation_events SET schema_version=99 WHERE operation_id=?1 AND sequence=0",
        "UPDATE operation_events SET payload='{\"event\":\"future_event\"}' WHERE operation_id=?1 AND sequence=0",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", directory.path().join("state.db").display());
        let id = crash(&url, "committed");
        let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
        sqlx::query(update)
            .bind(id.get())
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        let store = DurableStore::connect(&url).await.unwrap();
        let error = Runtime::new(store.clone())
            .run_operation(id, &NoInference, &tools())
            .await
            .unwrap_err();
        assert_eq!(error.code(), "corrupt_record");
        store.close().await;
    }
}

fn crash(url: &str, case: &str) -> OperationId {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tool_contract_crash_child",
            "--ignored",
            "--nocapture",
        ])
        .env("BRAINSTONE_CONTRACT_DB", url)
        .env("BRAINSTONE_CONTRACT_CASE", case)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(CRASH),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    OperationId::new(
        stdout
            .lines()
            .find_map(|line| line.strip_prefix("RECEIPT "))
            .unwrap()
            .parse()
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
#[ignore = "abrupt subprocess crash fixture"]
async fn tool_contract_crash_child() {
    let store = DurableStore::connect(&std::env::var("BRAINSTONE_CONTRACT_DB").unwrap())
        .await
        .unwrap();
    let case = std::env::var("BRAINSTONE_CONTRACT_CASE").unwrap();
    if case == "prior" {
        seed_prior(&store).await;
    }
    let prompt = if case == "legacy" {
        prompt("current").with_profile(ExecutionProfile::default())
    } else {
        prompt("current")
    };
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let claim = store.claim("contract-child", LEASE).await.unwrap().unwrap();
    let event = if case == "committed" {
        Event::Appended {
            message: Message::ToolResults(vec![result()]),
        }
    } else {
        Event::ToolFinished { result: result() }
    };
    store.record(&claim.lease, &event).await.unwrap();
    store
        .record(
            &claim.lease,
            &Event::Compacted {
                summary: "summary".into(),
                steps: 0,
            },
        )
        .await
        .unwrap();
    store
        .record(
            &claim.lease,
            &Event::Appended {
                message: Message::Assistant(response()),
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
async fn seed_prior(store: &DurableStore) {
    store.accept(&prompt("prior")).await.unwrap();
    let claim = store.claim("prior", LEASE).await.unwrap().unwrap();
    store
        .record(
            &claim.lease,
            &Event::Appended {
                message: Message::ToolResults(vec![result()]),
            },
        )
        .await
        .unwrap();
    store
        .settle(
            &claim.lease,
            &Outcome::Completed {
                text: "prior".into(),
            },
        )
        .await
        .unwrap();
}

struct NoEffect;
impl Tool for NoEffect {
    fn name(&self) -> &'static str {
        "Refund"
    }
    fn definition(&self) -> Value {
        serde_json::json!({"name":"Refund","description":"Never dispatched during terminal recovery","input_schema":{"type":"object"}})
    }
    fn call<'a>(
        &'a self,
        _: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        panic!("committed effect must not be dispatched again")
    }
}
fn tools() -> Registry {
    Registry::new(vec![Arc::new(NoEffect)])
}
