// These backend contract cases intentionally exceed the production function budget.
#![allow(clippy::too_many_lines)]
//! Session deletion has the same ownership boundary on both durable backends.
use brainstone_runtime::{
    Address, DurableStore, Prompt, Store, SubagentConfig, SubagentRequest, SubagentStore,
};
use std::time::Duration;

#[path = "support/durable.rs"]
mod contracts;
#[path = "support/database.rs"]
mod database;

fn prompt(surface: &str, conversation: &str, system: &str) -> Prompt {
    Prompt::new(
        Address::new(surface, conversation, "main").unwrap(),
        "initial",
        system,
        "work",
    )
    .unwrap()
}

#[tokio::test]
async fn sqlite_deletes_only_the_requested_session_tree() {
    let database = database::Database::sqlite();
    check(&database.url).await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_deletes_only_the_requested_session_tree() {
    let database = database::Database::postgres().await;
    check(&database.url).await;
    database.close().await;
}

async fn check(url: &str) {
    let store = DurableStore::connect(url).await.unwrap();
    let original = prompt("test", "reused", "old instructions");
    let parent = store.accept(&original).await.unwrap().operation;
    let config =
        SubagentConfig::new("specialist", "child", agent::ExecutionProfile::default()).unwrap();
    let request = SubagentRequest::new(parent.id, "child", "work", config).unwrap();
    let child = store.spawn_subagent(&request).await.unwrap().agent;
    let config =
        SubagentConfig::new("nested", "nested", agent::ExecutionProfile::default()).unwrap();
    let nested = SubagentRequest::new(child.operation.id, "nested", "work", config).unwrap();
    let grandchild = store.spawn_subagent(&nested).await.unwrap().agent.operation;
    let child_address =
        Address::new("subagent", &format!("agent-{}", child.id.get()), "main").unwrap();
    assert_eq!(
        store
            .delete_session(&child_address)
            .await
            .unwrap_err()
            .code(),
        "invalid_input"
    );
    refuses_child_lease(&store, original.address(), child.operation.id).await;
    let other = store
        .accept(&prompt("other", "reused", "other"))
        .await
        .unwrap()
        .operation;
    assert!(store.delete_session(original.address()).await.unwrap());
    assert!(store.operation(parent.id).await.unwrap().is_none());
    assert!(store.operation(child.operation.id).await.unwrap().is_none());
    assert!(store.operation(grandchild.id).await.unwrap().is_none());
    assert!(store.operation(other.id).await.unwrap().is_some());
    assert!(!store.delete_session(original.address()).await.unwrap());
    contracts::finish(&store, other.id).await;
    store.close().await;
    let reopened = DurableStore::connect(url).await.unwrap();
    let next = reopened
        .accept(&prompt("test", "reused", "new instructions"))
        .await
        .unwrap()
        .operation;
    assert_ne!(next.session_id, parent.session_id);
    assert_ne!(next.lane_id, parent.lane_id);
    refuses_active(&reopened, next.id).await;
    ancillary(&reopened).await;
    delivery_lease(&reopened).await;
    rollback(url, &reopened).await;
    payloads_and_approvals(&reopened).await;
    reopened.close().await;
}

async fn refuses_active(store: &DurableStore, id: brainstone_runtime::OperationId) {
    let claimed = store
        .claim_operation(id, "worker", Duration::from_secs(60))
        .await
        .unwrap()
        .unwrap();
    let address = Address::new("test", "reused", "main").unwrap();
    assert_eq!(
        store.delete_session(&address).await.unwrap_err().code(),
        "session_busy"
    );
    assert!(store.operation(id).await.unwrap().is_some());
    store
        .settle(
            &claimed.lease,
            &agent::Outcome::Completed {
                text: "done".into(),
            },
        )
        .await
        .unwrap();
    assert!(store.delete_session(&address).await.unwrap());
}

async fn refuses_child_lease(
    store: &DurableStore,
    address: &Address,
    id: brainstone_runtime::OperationId,
) {
    let claimed = store
        .claim_operation(id, "child-worker", Duration::from_secs(60))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        store.delete_session(address).await.unwrap_err().code(),
        "session_busy"
    );
    assert!(store.operation(id).await.unwrap().is_some());
    store
        .settle(
            &claimed.lease,
            &agent::Outcome::Completed {
                text: "done".into(),
            },
        )
        .await
        .unwrap();
}

async fn ancillary(store: &DurableStore) {
    contracts::ancillary(store, "delete-owned").await;
    contracts::schedules(store, "delete-owned").await;
    contracts::posts(store, "delete-owned").await;
    for surface in ["contract", "schedule"] {
        let address = Address::new(surface, "delete-owned", "lane").unwrap();
        assert!(store.delete_session(&address).await.unwrap());
    }
    let identity = agent::ExecutionIdentity {
        provider: "test".into(),
        tenant: "delete-owned".into(),
        actor: "owner".into(),
        conversation: "direct".into(),
        private: true,
    };
    let address = Address::new("post", &identity.conversation_key(), "origin").unwrap();
    assert!(store.delete_session(&address).await.unwrap());
}

async fn delivery_lease(store: &DurableStore) {
    use brainstone_runtime::{DeliveryCheckpoint, DeliveryClaim, DeliveryStore, DeliveryTarget};
    let prompt = prompt("test", "delivery", "system")
        .with_delivery(DeliveryTarget::new("delete-check", serde_json::json!({})).unwrap());
    let id = store.accept(&prompt).await.unwrap().operation.id;
    contracts::finish(store, id).await;
    let claim =
        DeliveryClaim::new("delete-check", "surface", Duration::from_secs(60), 100).unwrap();
    let delivery = store.claim_delivery(&claim).await.unwrap().unwrap();
    assert_eq!(
        store
            .delete_session(prompt.address())
            .await
            .unwrap_err()
            .code(),
        "session_busy"
    );
    let last = delivery.events.last().unwrap().sequence;
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(delivery.lease, last, None))
        .await
        .unwrap();
    assert!(store.delete_session(prompt.address()).await.unwrap());
}

async fn rollback(url: &str, store: &DurableStore) {
    let prompt = prompt("test", "rollback", "system");
    let id = store.accept(&prompt).await.unwrap().operation.id;
    failure_trigger(url, true).await;
    assert_eq!(
        store
            .delete_session(prompt.address())
            .await
            .unwrap_err()
            .code(),
        "store_failed"
    );
    assert!(store.operation(id).await.unwrap().is_some());
    failure_trigger(url, false).await;
    assert!(store.delete_session(prompt.address()).await.unwrap());
}

async fn failure_trigger(url: &str, create: bool) {
    if url.starts_with("sqlite:") {
        let pool = sqlx::SqlitePool::connect(url).await.unwrap();
        let statement = if create {
            "CREATE TRIGGER refuse_delete BEFORE DELETE ON lanes BEGIN SELECT RAISE(ABORT, 'injected failure'); END"
        } else {
            "DROP TRIGGER refuse_delete"
        };
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    } else {
        let pool = sqlx::PgPool::connect(url).await.unwrap();
        let statement = if create {
            "CREATE FUNCTION refuse_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected failure'; END; $$; CREATE TRIGGER refuse_delete BEFORE DELETE ON lanes FOR EACH ROW EXECUTE FUNCTION refuse_delete();"
        } else {
            "DROP TRIGGER refuse_delete ON lanes; DROP FUNCTION refuse_delete();"
        };
        sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    }
}

fn content() -> brainstone_runtime::PromptContent {
    brainstone_runtime::PromptContent {
        text: "attachments".into(),
        images: vec![agent::Image::new(agent::ImageMediaType::Png, vec![1], 1, 1).unwrap()],
    }
}

async fn payloads_and_approvals(store: &DurableStore) {
    let address = Address::new("test", "payloads", "main").unwrap();
    let prompt = Prompt::with_content(address.clone(), "initial", "system", content()).unwrap();
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let claim = store
        .claim_operation(id, "worker", Duration::from_secs(60))
        .await
        .unwrap()
        .unwrap();
    let steering = Prompt::with_content(address.clone(), "steer", "system", content()).unwrap();
    store.admit_with_tools(&steering, &[]).await.unwrap();
    store.steering(&claim.lease).await.unwrap();
    let call = agent::ToolCall {
        id: "one".into(),
        name: "External".into(),
        args: serde_json::json!({}),
    };
    let key = format!("operation/{}/step/0/call/one", id.get());
    let request = brainstone_runtime::ApprovalRequest::new(
        &claim,
        (&key, &call, &serde_json::json!({})),
        "policy",
    )
    .unwrap();
    store.authorize_tool(&claim.lease, &request).await.unwrap();
    assert!(store.approval(&request.id()).await.unwrap().is_some());
    store
        .settle(
            &claim.lease,
            &agent::Outcome::Completed {
                text: "done".into(),
            },
        )
        .await
        .unwrap();
    assert!(store.delete_session(&address).await.unwrap());
    assert!(store.approval(&request.id()).await.unwrap().is_none());
    assert!(store.operation(id).await.unwrap().is_none());
}
