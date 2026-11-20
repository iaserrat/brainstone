//! Authorization persists across recovery and cannot approve a changed invocation.
#![allow(clippy::too_many_lines)] // Scenario setup and immutable-binding assertions.
use brainstone_runtime::{Address, ApprovalRequest, ApprovalState, DurableStore, Prompt, Store};
use serde_json::json;
use std::time::Duration;
#[path = "support/database.rs"]
mod database;

#[tokio::test]
async fn sqlite_approval_is_bound_to_the_exact_invocation_and_survives_reopen() {
    let database = database::Database::sqlite();
    check(&database.url).await;
}
#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_approval_is_bound_to_the_exact_invocation_and_survives_reopen() {
    let database = database::Database::postgres().await;
    check(&database.url).await;
    database.close().await;
}
async fn check(url: &str) {
    let store = DurableStore::connect(url).await.unwrap();
    let prompt = Prompt::new(
        Address::new("approval", "session", "lane").unwrap(),
        "one",
        "system",
        "go",
    )
    .unwrap();
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let claim = store
        .claim_operation(id, "worker", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let request = ApprovalRequest::new(
        &claim,
        (
            "operation/1/step/0/call/one",
            &agent::ToolCall {
                id: "one".into(),
                name: "External".into(),
                args: json!({"x":1}),
            },
            &json!({}),
        ),
        "policy-a",
    )
    .unwrap();
    assert_eq!(
        backend(&store, &claim.lease, &request).await,
        ApprovalState::Pending
    );
    store
        .decide_approval(&request.id(), true, "operator")
        .await
        .unwrap();
    store
        .decide_approval(&request.id(), true, "operator")
        .await
        .unwrap();
    assert!(
        store
            .decide_approval(&request.id(), false, "operator")
            .await
            .is_err()
    );
    assert_eq!(
        store.authorize_tool(&claim.lease, &request).await.unwrap(),
        ApprovalState::Approved
    );
    let mut changed = ApprovalRequest::new(
        &claim,
        (
            "operation/1/step/0/call/one",
            &agent::ToolCall {
                id: "one".into(),
                name: "External".into(),
                args: json!({"x":2}),
            },
            &json!({}),
        ),
        "policy-a",
    )
    .unwrap();
    assert_ne!(request.id(), changed.id());
    assert_eq!(
        store.authorize_tool(&claim.lease, &changed).await.unwrap(),
        ApprovalState::Pending
    );
    changed.binding = "policy-b".into();
    assert_ne!(changed.id(), request.id());
    assert_eq!(
        store.authorize_tool(&claim.lease, &changed).await.unwrap(),
        ApprovalState::Pending
    );
    changed.definition = json!({"changed":true});
    assert_eq!(
        store.authorize_tool(&claim.lease, &changed).await.unwrap(),
        ApprovalState::Pending
    );
    store.close().await;
    let reopened = DurableStore::connect(url).await.unwrap();
    assert_eq!(
        reopened
            .authorize_tool(&claim.lease, &request)
            .await
            .unwrap(),
        ApprovalState::Approved
    );
    let page = reopened.operations(None, 1).await.unwrap();
    assert_eq!(page[0].id, id.get());
    assert_eq!(
        reopened
            .inspect(id)
            .await
            .unwrap()
            .unwrap()
            .pending_approvals,
        3
    );
    assert!(reopened.operations(None, 0).await.is_err());
    assert!(reopened.journal(id, -2, 100).await.is_err());
    assert_eq!(reopened.journal(id, -1, 100).await.unwrap(), []);
    reopened
        .heartbeat(&claim.lease, Duration::ZERO)
        .await
        .unwrap();
    assert!(matches!(
        reopened.authorize_tool(&claim.lease, &request).await,
        Err(brainstone_runtime::Error::LeaseLost)
    ));
    reopened.close().await;
}

async fn backend(
    store: &DurableStore,
    lease: &brainstone_runtime::Lease,
    request: &ApprovalRequest,
) -> ApprovalState {
    match store {
        DurableStore::Postgres(store) => store.authorize_tool(lease, request).await.unwrap(),
        DurableStore::Sqlite(store) => store.authorize_tool(lease, request).await.unwrap(),
    }
}
