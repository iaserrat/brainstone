use std::time::Duration;

use agent::ExecutionProfile;
use brainstone_runtime::{
    OperationId, Prompt, Store, Subagent, SubagentConfig, SubagentId, SubagentRequest,
    SubagentStore,
};

pub(crate) async fn check(store: &(impl Store + SubagentStore), prompt: &Prompt) {
    let (parent, child) = spawn(store, prompt).await;
    finish(store, child.operation.id).await;
    let first = send(store, parent, child.id, "one").await;
    finish(store, first.operation.id).await;
    let second = send(store, parent, child.id, "two").await;
    for (key, id) in [("one", first.operation.id), ("two", second.operation.id)] {
        let replay = send(store, parent, child.id, key).await;
        assert_eq!(Some(replay.operation), store.operation(id).await.unwrap());
        let current = store.subagent(parent, child.id).await.unwrap();
        assert_eq!(current, Some(second.clone()), "current task");
    }
    let conflict = store
        .message_subagent(parent, child.id, "one", "changed")
        .await;
    assert_eq!(
        conflict.unwrap_err(),
        brainstone_runtime::Error::IdempotencyConflict
    );
    let unchanged = store.subagent(parent, child.id).await.unwrap();
    assert_eq!(unchanged, Some(second.clone()));
    finish(store, second.operation.id).await;
    finish(store, parent).await;
}

async fn spawn(store: &(impl Store + SubagentStore), prompt: &Prompt) -> (OperationId, Subagent) {
    let parent = store.accept(prompt).await.unwrap().operation.id;
    let config = SubagentConfig::new("child", "child system", ExecutionProfile::default()).unwrap();
    let request = SubagentRequest::new(parent, "spawn", "start", config).unwrap();
    let child = store.spawn_subagent(&request).await.unwrap().agent;
    (parent, child)
}

async fn send(
    store: &impl SubagentStore,
    parent: OperationId,
    child: SubagentId,
    key: &str,
) -> Subagent {
    store
        .message_subagent(parent, child, key, key)
        .await
        .unwrap()
}

async fn finish(store: &impl Store, id: OperationId) {
    let claim = store
        .claim_operation(id, "finish", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let outcome = agent::Outcome::Completed {
        text: "done".to_owned(),
    };
    store.settle(&claim.lease, &outcome).await.unwrap();
}
