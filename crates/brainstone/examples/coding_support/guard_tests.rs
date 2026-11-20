//! Only committed successful checks block future edits; pending checks cannot share a batch.
use super::*;
use brainstone::agent::{self, ModelResponse, StopReason, ToolCall, ToolResult};
use brainstone::runtime::{self, Store};
use serde_json::json;

fn assistant(names: &[&str]) -> Event {
    Event::Appended {
        message: Message::Assistant(ModelResponse {
            reasoning: vec![],
            text: String::new(),
            stop: StopReason::ToolUse,
            model: None,
            tool_calls: names
                .iter()
                .map(|name| ToolCall {
                    id: (*name).into(),
                    name: (*name).into(),
                    args: json!({}),
                })
                .collect(),
        }),
    }
}

fn result(is_error: bool) -> Event {
    Event::Appended {
        message: Message::ToolResults(vec![ToolResult {
            id: "check".into(),
            name: "RunChecks".into(),
            content: "result".into(),
            is_error,
            images: vec![],
        }]),
    }
}

#[test]
fn mixed_batches_reject_both_orders_and_later_separate_edits_are_allowed() {
    for names in [["RunChecks", "EditFile"], ["EditFile", "RunChecks"]] {
        let mut mixed = false;
        inspect(&[(0, assistant(&names))], &mut mixed).unwrap();
        assert!(mixed);
        inspect(
            &[(1, result(true)), (2, assistant(&["EditFile"]))],
            &mut mixed,
        )
        .unwrap();
        assert!(!mixed);
    }
}

#[test]
fn successful_committed_checks_block_edits_but_provisional_events_do_not() {
    let event = result(false);
    assert!(matches!(
        inspect(&[(0, event)], &mut false),
        Err(Error::ChecksAlreadyPassed)
    ));
    let Event::Appended {
        message: Message::ToolResults(mut results),
    } = result(false)
    else {
        panic!("fixture")
    };
    let provisional = Event::ToolFinished {
        result: results.remove(0),
    };
    assert!(inspect(&[(0, provisional)], &mut false).is_ok());
}

async fn setup(root: &std::path::Path) -> (DurableStore, runtime::Claimed) {
    std::fs::write(root.join("lib.rs"), "before").unwrap();
    let store = DurableStore::connect(&format!("sqlite://{}", root.join("state.db").display()))
        .await
        .unwrap();
    let prompt = brainstone::Prompt::new(
        brainstone::Address::new("coding", "test", "main").unwrap(),
        "one",
        "system",
        "edit",
    )
    .unwrap();
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let claim = store
        .claim_operation(id, "test", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    (store, claim)
}

async fn edit_call(
    store: &DurableStore,
    claim: &runtime::Claimed,
    root: &std::path::Path,
) -> ToolResult {
    let tool = super::super::edit::tool(
        crate::workspace::Workspace::open(root).unwrap(),
        store.clone(),
        false,
    )
    .unwrap();
    let registry = brainstone::Registry::default().with_tool(tool);
    let call = ToolCall {
        id: "edit".into(),
        name: "EditFile".into(),
        args: json!({"path":"lib.rs","expected":"before","replacement":"after"}),
    };
    let context = agent::ToolContext {
        operation_id: u64::try_from(claim.operation.id.get()).unwrap(),
        identity: None,
        profile: claim.thread.profile().clone(),
    };
    agent::execute_tool_with_context(
        &call,
        &registry,
        Some("durable-test-key".into()),
        Some(context),
    )
    .await
}

#[tokio::test]
async fn both_mixed_batch_orders_leave_the_file_unchanged() {
    for names in [["RunChecks", "EditFile"], ["EditFile", "RunChecks"]] {
        let root = tempfile::tempdir().unwrap();
        let (store, claim) = setup(root.path()).await;
        store
            .record(&claim.lease, &assistant(&names))
            .await
            .unwrap();
        let result = edit_call(&store, &claim, root.path()).await;
        assert!(result.is_error);
        let document: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(document["error"], "coding_checks_order");
        assert_eq!(
            std::fs::read_to_string(root.path().join("lib.rs")).unwrap(),
            "before"
        );
        store.close().await;
    }
}

#[tokio::test]
async fn committed_checks_beyond_the_first_journal_page_prevent_later_edits() {
    let root = tempfile::tempdir().unwrap();
    let (store, claim) = setup(root.path()).await;
    for _ in 0..PAGE_SIZE {
        store
            .record(&claim.lease, &Event::RunStarted)
            .await
            .unwrap();
    }
    store.record(&claim.lease, &result(false)).await.unwrap();
    store
        .record(&claim.lease, &assistant(&["EditFile"]))
        .await
        .unwrap();
    let result = edit_call(&store, &claim, root.path()).await;
    let document: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    assert_eq!(document["error"], "coding_checks_already_passed");
    assert_eq!(
        std::fs::read_to_string(root.path().join("lib.rs")).unwrap(),
        "before"
    );
    store.close().await;
}

#[tokio::test]
async fn a_failed_check_allows_a_later_separate_edit() {
    let root = tempfile::tempdir().unwrap();
    let (store, claim) = setup(root.path()).await;
    store
        .record(&claim.lease, &assistant(&["RunChecks"]))
        .await
        .unwrap();
    store.record(&claim.lease, &result(true)).await.unwrap();
    store
        .record(&claim.lease, &assistant(&["EditFile"]))
        .await
        .unwrap();
    let edited = edit_call(&store, &claim, root.path()).await;
    assert!(!edited.is_error, "{}", edited.content);
    assert_eq!(
        std::fs::read_to_string(root.path().join("lib.rs")).unwrap(),
        "after"
    );
    store.close().await;
}
