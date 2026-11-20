#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
// Table-driven durable security contract exercises both stores.
#![allow(clippy::too_many_lines)]
use agent::ExecutionIdentity;
use brainstone_runtime::{Address, MemoryStore, Prompt, Store};
use std::time::Duration;

fn identity(actor: &str) -> ExecutionIdentity {
    ExecutionIdentity {
        provider: "test".into(),
        tenant: "tenant".into(),
        actor: actor.into(),
        conversation: "conversation".into(),
        private: true,
    }
}

fn prompt(actor: &str, request: &str) -> Prompt {
    Prompt::new(
        Address::new("test", "conversation", "lane").unwrap(),
        request,
        "system",
        "task",
    )
    .unwrap()
    .with_identity(Some(identity(actor)))
}

#[tokio::test]
async fn reclaim_preserves_the_accepted_identity() {
    let store = MemoryStore::default();
    let accepted = store.accept(&prompt("alice", "one")).await.unwrap();
    let first = store.claim("first", Duration::ZERO).await.unwrap().unwrap();
    assert_eq!(first.identity, Some(identity("alice")));
    let retry = store
        .claim("restart", Duration::from_secs(10))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retry.operation.id, accepted.operation.id);
    assert_eq!(retry.identity, first.identity);
}

#[tokio::test]
async fn duplicate_cannot_replace_the_authenticated_caller() {
    let store = MemoryStore::default();
    store.accept(&prompt("alice", "one")).await.unwrap();
    assert!(store.accept(&prompt("bob", "one")).await.is_err());
}

use brainstone_runtime::{
    Memories, ScheduleRequest, ScheduleStore, ScheduleTime, SubagentConfig, SubagentRequest,
    SubagentStore,
};

async fn finish(store: &impl Store, id: brainstone_runtime::OperationId) {
    let claim = store
        .claim_operation(id, "finish", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    store
        .settle(
            &claim.lease,
            &agent::Outcome::Completed {
                text: "done".into(),
            },
        )
        .await
        .unwrap();
}

async fn delegation_and_schedule(
    store: &(impl Store + SubagentStore + ScheduleStore + Memories),
    prefix: &str,
) {
    let mut parent = prompt("alice", "parent");
    parent = Prompt::new(
        Address::new("test", prefix, "lane").unwrap(),
        "parent",
        "system",
        "task",
    )
    .unwrap()
    .with_identity(parent.identity().cloned());
    let accepted = store.accept(&parent).await.unwrap().operation.id;
    let config =
        SubagentConfig::new("child", "system", agent::ExecutionProfile::default()).unwrap();
    let request = SubagentRequest::new(accepted, "spawn", "task", config).unwrap();
    let child = store.spawn_subagent(&request).await.unwrap().agent;
    child_retry(store, child.operation.id, &parent).await;
    let follow = store
        .message_subagent(accepted, child.id, "follow", "continue")
        .await
        .unwrap();
    child_retry(store, follow.operation.id, &parent).await;
    scheduled_identity(store, accepted, &parent).await;
}

async fn child_retry(
    store: &(impl Store + Memories),
    id: brainstone_runtime::OperationId,
    parent: &Prompt,
) {
    let claim = store
        .claim_operation(id, "child", Duration::ZERO)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claim.identity.as_ref(), parent.identity());
    let reclaimed = store
        .claim_operation(id, "retry", Duration::ZERO)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reclaimed.identity, claim.identity);
    assert_eq!(store.memory_scope(id).await.unwrap(), parent.memory_scope());
    finish(store, id).await;
}

async fn scheduled_identity(
    store: &(impl Store + ScheduleStore),
    accepted: brainstone_runtime::OperationId,
    parent: &Prompt,
) {
    let schedule = ScheduleRequest::new(
        accepted,
        "later",
        "scheduled",
        ScheduleTime::AfterSeconds(0),
    )
    .unwrap();
    store.schedule(&schedule).await.unwrap();
    finish(store, accepted).await;
    let scheduled = store
        .claim("scheduler", Duration::ZERO)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(scheduled.identity.as_ref(), parent.identity());
    assert_eq!(
        scheduled.operation.lane_id,
        store.operation(accepted).await.unwrap().unwrap().lane_id
    );
    finish(store, scheduled.operation.id).await;
}

#[tokio::test]
async fn child_retry_followup_and_schedule_keep_the_original_principal() {
    delegation_and_schedule(&MemoryStore::default(), "delegation").await;
}

async fn shared_callers(store: &(impl Store + SubagentStore), prefix: &str) {
    let (a, b) = shared_prompts(prefix);
    let first = store.accept(&a).await.unwrap().operation.id;
    let claim = store
        .claim_operation(first, "alice", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let admitted = store.admit_with_tools(&b, &[]).await.unwrap();
    assert!(!matches!(
        admitted,
        brainstone_runtime::Admitted::Steering { .. }
    ));
    assert_eq!(store.steering(&claim.lease).await.unwrap(), []);
    let second = store.accept(&b).await.unwrap().operation.id;
    reject_child_control(store, first, second).await;
    store
        .settle(
            &claim.lease,
            &agent::Outcome::Completed {
                text: "done".into(),
            },
        )
        .await
        .unwrap();
    finish(store, second).await;
}

#[tokio::test]
async fn shared_conversation_cannot_steer_or_control_another_callers_authority() {
    shared_callers(&MemoryStore::default(), "shared").await;
}

#[tokio::test]
#[ignore = "requires dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_identity_survives_reopening_and_inheritance() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").unwrap();
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .to_string();
    let (url, admin, schema) = isolated_schema(&url, &suffix).await;
    let store = brainstone_runtime::PostgresStore::connect(&url)
        .await
        .unwrap();
    let parent = Prompt::new(
        Address::new("identity", &suffix, "restart").unwrap(),
        "one",
        "system",
        "task",
    )
    .unwrap()
    .with_identity(Some(identity("alice")));
    let accepted = store.accept(&parent).await.unwrap().operation.id;
    let first = store
        .claim_operation(accepted, "before-crash", Duration::ZERO)
        .await
        .unwrap()
        .unwrap();
    drop(store);
    let reopened = brainstone_runtime::PostgresStore::connect(&url)
        .await
        .unwrap();
    let retry = reopened
        .claim_operation(accepted, "after-crash", Duration::ZERO)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.identity, retry.identity);
    assert_eq!(retry.identity.as_ref(), parent.identity());
    finish(&reopened, accepted).await;
    delegation_and_schedule(&reopened, &format!("delegate-{suffix}")).await;
    shared_callers(&reopened, &format!("shared-{suffix}")).await;
    background_identity(&reopened, &format!("background-{suffix}")).await;
    reopened.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
}

fn shared_prompts(prefix: &str) -> (Prompt, Prompt) {
    let address = Address::new("test", prefix, "shared").unwrap();
    let mut alice = identity("alice");
    alice.private = false;
    let mut bob = alice.clone();
    bob.actor = "bob".into();
    let a = Prompt::new(address.clone(), "a", "system", "first")
        .unwrap()
        .with_identity(Some(alice));
    let b = Prompt::new(address, "b", "system", "second")
        .unwrap()
        .with_identity(Some(bob));
    (a, b)
}

async fn reject_child_control(
    store: &(impl Store + SubagentStore),
    first: brainstone_runtime::OperationId,
    second: brainstone_runtime::OperationId,
) {
    let config =
        SubagentConfig::new("child", "system", agent::ExecutionProfile::default()).unwrap();
    let child = store
        .spawn_subagent(&SubagentRequest::new(first, "child", "task", config).unwrap())
        .await
        .unwrap()
        .agent;
    assert!(
        store
            .message_subagent(second, child.id, "attack", "act as alice")
            .await
            .is_err()
    );
    assert!(store.subagent(second, child.id).await.is_err());
    assert!(store.stop_subagent(second, child.id).await.is_err());
    finish(store, child.operation.id).await;
}

struct Capture(std::sync::Mutex<Vec<agent::ToolRequest>>);
impl agent::Tool for Capture {
    fn name(&self) -> &'static str {
        "capture"
    }
    fn definition(&self) -> serde_json::Value {
        serde_json::json!({"name":"capture", "description":"capture", "input_schema":{"type":"object"}})
    }
    fn call<'a>(
        &'a self,
        request: agent::ToolRequest,
    ) -> std::pin::Pin<
        Box<dyn Future<Output = Result<agent::ToolOutput, agent::ToolError>> + Send + 'a>,
    > {
        Box::pin(async move {
            self.0.lock().unwrap().push(request);
            Ok("done".to_owned().into())
        })
    }
}
struct Done;
impl agent::Model for Done {
    async fn generate(&self, _: &agent::Thread, _: &[serde_json::Value]) -> agent::ModelResponse {
        agent::ModelResponse {
            model: None,
            text: "done".into(),
            reasoning: vec![],
            tool_calls: vec![],
            stop: agent::StopReason::EndTurn,
        }
    }
}

#[tokio::test]
async fn recovered_tool_execution_keeps_identity_and_retry_key_outside_arguments() {
    let store = MemoryStore::default();
    let tool = std::sync::Arc::new(Capture(std::sync::Mutex::new(Vec::new())));
    let tools = agent::Registry::new(vec![tool.clone()]);
    let runtime = brainstone_runtime::Runtime::new(store.clone());
    let accepted = store
        .accept_with_tools(&prompt("alice", "replay"), &tools.definitions())
        .await
        .unwrap();
    let claimed = store
        .claim("crashing", Duration::from_millis(50))
        .await
        .unwrap()
        .unwrap();
    let call = agent::ToolCall {
        id: "call".into(),
        name: "capture".into(),
        args: serde_json::json!({"actor":"victim"}),
    };
    let response = agent::ModelResponse {
        model: None,
        text: String::new(),
        reasoning: vec![],
        tool_calls: vec![call.clone()],
        stop: agent::StopReason::ToolUse,
    };
    store
        .record(
            &claimed.lease,
            &agent::Event::Appended {
                message: agent::Message::Assistant(response),
            },
        )
        .await
        .unwrap();
    store
        .record(
            &claimed.lease,
            &agent::Event::ToolStarted {
                id: call.id,
                name: call.name,
                args: call.args,
            },
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(60)).await;
    runtime.run_next(&Done, &tools).await.unwrap();
    let calls = tool.0.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].context.as_ref().unwrap().identity,
        Some(identity("alice"))
    );
    assert_eq!(calls[0].args["actor"], "victim");
    assert!(
        calls[0]
            .idempotency_key
            .as_ref()
            .unwrap()
            .starts_with(&format!("operation/{}/", accepted.operation.id.get()))
    );
}

async fn background_identity(store: &(impl Store + SubagentStore), prefix: &str) {
    let parent = Prompt::new(
        Address::new("test", prefix, "background").unwrap(),
        "parent",
        "system",
        "task",
    )
    .unwrap()
    .with_identity(Some(identity("alice")));
    let accepted = store.accept(&parent).await.unwrap().operation.id;
    let config = SubagentConfig::new("child", "system", agent::ExecutionProfile::default())
        .unwrap()
        .in_background(true);
    let request = SubagentRequest::new(accepted, "background", "task", config).unwrap();
    let child = store.spawn_subagent(&request).await.unwrap().agent;
    finish(store, accepted).await;
    finish(store, child.operation.id).await;
    let notification = store
        .claim("notification", Duration::ZERO)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(notification.identity.as_ref(), parent.identity());
    finish(store, notification.operation.id).await;
}

#[tokio::test]
async fn background_completion_preserves_parent_identity() {
    background_identity(&MemoryStore::default(), "background").await;
}

async fn isolated_schema(url: &str, suffix: &str) -> (String, sqlx::PgPool, String) {
    let admin = sqlx::PgPool::connect(url).await.unwrap();
    // Only a generated numeric suffix enters the SQL identifier.
    assert!(!suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()));
    let schema = format!("identity_test_{suffix}");
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin)
        .await
        .unwrap();
    let separator = if url.contains('?') { '&' } else { '?' };
    (
        format!("{url}{separator}options=-csearch_path%3D{schema}"),
        admin,
        schema,
    )
}
