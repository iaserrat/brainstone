//! Cross-session memory behavior through the public store contract.

use brainstone_runtime::{Address, Memories, Memory, MemoryStore, OperationId, Prompt, Store};

fn memory(scope: &str, key: &str, content: &str) -> Memory {
    Memory::new(scope, key, content).expect("valid memory")
}

#[tokio::test]
async fn recall_returns_newest_first_within_scope_and_limit() {
    let store = MemoryStore::default();
    for (key, content) in [("k1", "first"), ("k2", "second"), ("k3", "third")] {
        store
            .save_memory(&memory("test/C1", key, content))
            .await
            .expect("saved");
    }
    store
        .save_memory(&memory("test/C2", "k4", "elsewhere"))
        .await
        .expect("saved");

    let recalled = store
        .recall_memories("test/C1", None, 2)
        .await
        .expect("recalled");
    assert_eq!(recalled, vec!["third".to_owned(), "second".to_owned()]);
}

#[tokio::test]
async fn saving_the_same_key_twice_is_idempotent() {
    let store = MemoryStore::default();
    let entry = memory("test/C1", "k1", "prefers metric units");
    store.save_memory(&entry).await.expect("saved");
    store.save_memory(&entry).await.expect("replay is a no-op");

    let recalled = store
        .recall_memories("test/C1", None, 10)
        .await
        .expect("recalled");
    assert_eq!(recalled, vec!["prefers metric units".to_owned()]);
}

#[tokio::test]
async fn reusing_a_key_with_different_content_is_a_conflict() {
    let store = MemoryStore::default();
    store
        .save_memory(&memory("test/C1", "k1", "original"))
        .await
        .expect("saved");
    let error = store
        .save_memory(&memory("test/C1", "k1", "rewritten"))
        .await
        .expect_err("conflict");
    assert_eq!(error.code(), "idempotency_conflict");
}

#[tokio::test]
async fn recall_filters_by_case_insensitive_substring() {
    let store = MemoryStore::default();
    for (key, content) in [
        ("k1", "prefers Métric units"),
        ("k2", "keeps notes offline"),
        ("k3", "likes METRIC tables"),
    ] {
        store
            .save_memory(&memory("test/C1", key, content))
            .await
            .expect("saved");
    }

    let recalled = store
        .recall_memories("test/C1", Some("metric"), 10)
        .await
        .expect("recalled");
    assert_eq!(recalled, vec!["likes METRIC tables".to_owned()]);
    let unicode = store
        .recall_memories("test/C1", Some("métric"), 10)
        .await
        .expect("recalled");
    assert_eq!(unicode, vec!["prefers Métric units".to_owned()]);
}

#[tokio::test]
async fn memory_scope_resolves_an_operation_to_its_owning_identity() {
    let store = MemoryStore::default();
    let address = Address::new("test", "C42", "thread-1").expect("address");
    let prompt = Prompt::new(address, "event-1", "be useful", "hello")
        .expect("prompt")
        .with_actor("U7");
    let accepted = store.accept(&prompt).await.expect("accepted");

    let scope = store
        .memory_scope(accepted.operation.id)
        .await
        .expect("scope");
    assert_eq!(scope.surface, "test");
    assert_eq!(scope.conversation, "C42");
    assert_eq!(scope.actor.as_deref(), Some("U7"));
}

#[tokio::test]
async fn memory_scope_has_no_actor_when_the_surface_supplied_none() {
    let store = MemoryStore::default();
    let address = Address::new("test", "C42", "thread-1").expect("address");
    let prompt = Prompt::new(address, "event-1", "be useful", "hello").expect("prompt");
    let accepted = store.accept(&prompt).await.expect("accepted");

    let scope = store
        .memory_scope(accepted.operation.id)
        .await
        .expect("scope");
    assert_eq!(scope.actor, None);
}

#[tokio::test]
async fn memory_scope_rejects_an_unknown_operation() {
    let store = MemoryStore::default();
    let missing = OperationId::new(999).expect("id");
    let error = store
        .memory_scope(missing)
        .await
        .expect_err("unknown operation");
    assert_eq!(error.code(), "invalid_input");
}

#[test]
fn a_memory_rejects_empty_identity_or_content() {
    for (scope, key, content) in [("", "k", "c"), ("s", "", "c"), ("s", "k", "")] {
        let error = Memory::new(scope, key, content).expect_err("invalid");
        assert_eq!(error.code(), "invalid_input");
    }
}
