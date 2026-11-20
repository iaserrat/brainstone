//! Steering and crash recovery must never alias distinct external effects.
use agent::{
    Message, Model, ModelResponse, Registry, StopReason, Thread, Tool, ToolCall, ToolError,
    ToolOutput, ToolRequest,
};
use brainstone_runtime::{Address, CompactionPolicy, Config, Prompt, Runtime, SqliteStore, Store};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::{pin::Pin, sync::Arc, time::Duration};

const LEASE: Duration = Duration::from_secs(30);
const CRASH_LEASE: Duration = Duration::from_secs(5);
const RECOVERY_WAIT: Duration = Duration::from_secs(6);
const CRASH: i32 = 73;
const SUMMARY_REQUEST: &str = "Summarize the task.";
const COMPACTION_TOKENS: usize = 1000;
const LARGE_RESULT_REPETITIONS: usize = 2048;

struct SteeringModel;

impl Model for SteeringModel {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "The regression model is deterministic."
    )]
    async fn generate(&self, thread: &Thread, _: &[Value]) -> ModelResponse {
        let calls = match thread.last() {
            Message::User(text) if text == SUMMARY_REQUEST => Vec::new(),
            Message::User(text) | Message::Summary(text) => vec![ToolCall {
                id: "same-response-local-id".to_owned(),
                name: "RecordValue".to_owned(),
                args: json!({"value":text}),
            }],
            _ => Vec::new(),
        };
        let stop = if calls.is_empty() {
            StopReason::EndTurn
        } else {
            StopReason::ToolUse
        };
        ModelResponse {
            reasoning: Vec::new(),
            text: "second".to_owned(),
            tool_calls: calls,
            stop,
            model: None,
        }
    }
}

struct RecordValue {
    store: SqliteStore,
    effects: SqlitePool,
    crash: bool,
}

impl Tool for RecordValue {
    fn name(&self) -> &'static str {
        "RecordValue"
    }
    fn definition(&self) -> Value {
        json!({"name":self.name(), "description":"Record the value", "input_schema":{"type":"object"}})
    }
    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move { Ok(self.record(request).await) })
    }
}

impl RecordValue {
    async fn record(&self, request: ToolRequest) -> ToolOutput {
        let key = request.idempotency_key.expect("durable key");
        let value = request.args["value"].as_str().unwrap();
        self.apply_effect(&key, value).await;
        if value == "first" {
            self.store
                .admit_with_tools(&prompt("steering", "second"), &[])
                .await
                .unwrap();
        } else if self.crash {
            std::process::exit(CRASH);
        }
        let text = if value == "first" {
            value.repeat(LARGE_RESULT_REPETITIONS)
        } else {
            value.to_owned()
        };
        ToolOutput::from(text)
    }

    async fn apply_effect(&self, key: &str, value: &str) {
        sqlx::query("INSERT INTO effects VALUES(?1,?2) ON CONFLICT DO NOTHING")
            .bind(key)
            .bind(value)
            .execute(&self.effects)
            .await
            .unwrap();
        let stored: String = sqlx::query_scalar("SELECT value FROM effects WHERE retry_key=?1")
            .bind(key)
            .fetch_one(&self.effects)
            .await
            .unwrap();
        assert_eq!(
            stored, value,
            "distinct calls must not alias the same durable key"
        );
    }
}

fn prompt(id: &str, text: &str) -> Prompt {
    Prompt::new(
        Address::new("test", "identity", "main").unwrap(),
        id,
        "system",
        text,
    )
    .unwrap()
}

async fn effects(url: &str) -> SqlitePool {
    let pool = SqlitePool::connect(url).await.unwrap();
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("PRAGMA synchronous=FULL")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS effects(retry_key TEXT PRIMARY KEY,value TEXT NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

fn runtime(store: SqliteStore, crash: bool) -> Runtime<SqliteStore> {
    let policy = CompactionPolicy::new(COMPACTION_TOKENS, SUMMARY_REQUEST).unwrap();
    let lease = if crash { CRASH_LEASE } else { LEASE };
    let config = Config::new("identity", lease)
        .unwrap()
        .with_compaction(policy);
    Runtime::with_config(store, config)
}

async fn execute(database: &str, effects: SqlitePool, crash: bool) {
    let store = SqliteStore::connect(database).await.unwrap();
    let runtime = runtime(store.clone(), crash);
    let tools = Registry::new(vec![Arc::new(RecordValue {
        store: store.clone(),
        effects,
        crash,
    })]);
    let accepted = runtime.accept(&prompt("original", "first")).await.unwrap();
    let operation = runtime
        .run_operation(accepted.operation.id, &SteeringModel, &tools)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        operation.outcome,
        Some(agent::Outcome::Completed { .. })
    ));
    store.close().await;
}

#[test]
fn sqlite_keeps_distinct_effect_keys_after_steering_and_abrupt_exit() {
    let directory = tempfile::tempdir().unwrap();
    let database = format!("sqlite://{}", directory.path().join("runtime.db").display());
    let ledger = format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("effects.db").display()
    );
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "identity_crash_child", "--ignored"])
        .env("BRAINSTONE_IDENTITY_DATABASE", &database)
        .env("BRAINSTONE_IDENTITY_EFFECTS", &ledger)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(CRASH));
    std::thread::sleep(RECOVERY_WAIT);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(recover(&database, &ledger));
}

async fn recover(database: &str, ledger: &str) {
    let pool = effects(ledger).await;
    execute(database, pool.clone(), false).await;
    assert_compacted(database).await;
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT retry_key,value FROM effects ORDER BY value")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].0, rows[1].0);
    assert!(rows[0].0.contains("/step/1/"));
    assert!(rows[1].0.contains("/step/2/"));
    pool.close().await;
}

async fn assert_compacted(database: &str) {
    let pool = SqlitePool::connect(database).await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM operation_events WHERE payload->>'event'='compacted'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(count > 0, "the recovery must cross a compaction boundary");
    pool.close().await;
}

#[tokio::test]
#[ignore = "subprocess crash fixture"]
async fn identity_crash_child() {
    let database = std::env::var("BRAINSTONE_IDENTITY_DATABASE").unwrap();
    let ledger = std::env::var("BRAINSTONE_IDENTITY_EFFECTS").unwrap();
    execute(&database, effects(&ledger).await, true).await;
    panic!("fixture should exit after the second effect commits")
}
