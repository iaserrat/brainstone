#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Memory capability behavior: tools on the tool seam and prompt injection.

use std::sync::{Arc, Mutex};

use agent::{ExecutionProfile, Tool, ToolContext, ToolRequest};
use brainstone_memory::{Config, MemoryAdmission, RecallTool, SaveTool, Scope};
use host::Admission;
use runtime::{Address, Memories, Memory, MemoryStore, OperationId, Prompt, Store};

#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<Vec<Prompt>>>);

impl Admission for Recorder {
    async fn accept(&self, prompt: &Prompt) -> Result<OperationId, host::Error> {
        self.0
            .lock()
            .expect("recorder poisoned")
            .push(prompt.clone());
        Ok(OperationId::new(1).expect("operation id"))
    }
}

impl Recorder {
    fn accepted_text(&self) -> String {
        let prompts = self.0.lock().expect("recorder poisoned");
        prompts
            .last()
            .expect("one accepted prompt")
            .text()
            .to_owned()
    }
}

fn prompt(conversation: &str, text: &str) -> Prompt {
    let address = Address::new("test", conversation, "thread-1").expect("address");
    Prompt::new(address, "event-1", "be useful", text).expect("prompt")
}

async fn accepted_operation(store: &MemoryStore, conversation: &str) -> u64 {
    let accepted = store
        .accept(&prompt(conversation, "hello"))
        .await
        .expect("accepted");
    u64::try_from(accepted.operation.id.get()).expect("positive id")
}

async fn accepted_actor_operation(store: &MemoryStore, conversation: &str, actor: &str) -> u64 {
    let accepted = store
        .accept(&prompt(conversation, "hello").with_actor(actor))
        .await
        .expect("accepted");
    u64::try_from(accepted.operation.id.get()).expect("positive id")
}

fn request(args: serde_json::Value, operation_id: Option<u64>, key: Option<&str>) -> ToolRequest {
    ToolRequest {
        args,
        idempotency_key: key.map(str::to_owned),
        context: operation_id.map(|operation_id| ToolContext {
            identity: None,
            operation_id,
            profile: ExecutionProfile::default(),
        }),
    }
}

async fn seed(store: &MemoryStore, scope: &str, key: &str, content: &str) {
    let memory = Memory::new(scope, key, content).expect("memory");
    store.save_memory(&memory).await.expect("seeded");
}

#[tokio::test]
async fn memory_save_persists_content_for_the_conversation_scope() {
    let store = MemoryStore::default();
    let operation = accepted_operation(&store, "C42").await;
    let tool = SaveTool::new(store.clone(), Scope::Conversation);

    let args = serde_json::json!({"content": "prefers metric units"});
    let result = tool
        .call(request(args, Some(operation), Some("op/1/step/0/call/a")))
        .await
        .expect("saved");
    assert!(result.text.contains("saved"));

    let recalled = store
        .recall_memories("test/C42", None, 10)
        .await
        .expect("recalled");
    assert_eq!(recalled, vec!["prefers metric units".to_owned()]);
}

#[tokio::test]
async fn a_replayed_memory_save_stores_one_memory() {
    let store = MemoryStore::default();
    let operation = accepted_operation(&store, "C42").await;
    let tool = SaveTool::new(store.clone(), Scope::Conversation);
    let args = serde_json::json!({"content": "prefers metric units"});

    for _ in 0..2 {
        tool.call(request(
            args.clone(),
            Some(operation),
            Some("op/1/step/0/call/a"),
        ))
        .await
        .expect("saved");
    }

    let recalled = store
        .recall_memories("test/C42", None, 10)
        .await
        .expect("recalled");
    assert_eq!(recalled.len(), 1);
}

#[tokio::test]
async fn memory_save_requires_a_durable_context() {
    let store = MemoryStore::default();
    let tool = SaveTool::new(store, Scope::Conversation);
    let args = serde_json::json!({"content": "x"});

    let missing_context = tool
        .call(request(args.clone(), None, Some("key")))
        .await
        .expect_err("no context");
    assert_eq!(missing_context.code, "durable_context_required");
}

#[tokio::test]
async fn memory_save_rejects_malformed_arguments() {
    let store = MemoryStore::default();
    let operation = accepted_operation(&store, "C42").await;
    let tool = SaveTool::new(store, Scope::Conversation);

    let error = tool
        .call(request(serde_json::json!({}), Some(operation), Some("key")))
        .await
        .expect_err("missing content");
    assert_eq!(error.code, "invalid_memory_arguments");
}

#[tokio::test]
async fn memory_recall_searches_the_resolved_scope() {
    let store = MemoryStore::default();
    let operation = accepted_operation(&store, "C42").await;
    seed(&store, "test/C42", "k1", "prefers metric units").await;
    seed(&store, "test/C42", "k2", "keeps notes offline").await;
    seed(&store, "test/C1", "k3", "metric elsewhere").await;
    let tool = RecallTool::new(store, Scope::Conversation, 10);

    let result = tool
        .call(request(
            serde_json::json!({"query": "metric"}),
            Some(operation),
            Some("key"),
        ))
        .await
        .expect("recalled");
    assert!(result.text.contains("prefers metric units"));
    assert!(!result.text.contains("keeps notes offline"));
    assert!(!result.text.contains("metric elsewhere"));
}

#[tokio::test]
async fn surface_scope_shares_memories_across_conversations() {
    let store = MemoryStore::default();
    let operation = accepted_operation(&store, "C42").await;
    seed(&store, "test", "k1", "org-wide fact").await;
    let tool = RecallTool::new(store, Scope::Surface, 10);

    let result = tool
        .call(request(serde_json::json!({}), Some(operation), Some("key")))
        .await
        .expect("recalled");
    assert!(result.text.contains("org-wide fact"));
}

#[tokio::test]
async fn user_scope_keeps_each_actors_memories_apart_across_conversations() {
    let store = MemoryStore::default();
    let operation = accepted_actor_operation(&store, "C42", "U7").await;
    let save = SaveTool::new(store.clone(), Scope::User);
    let args = serde_json::json!({"content": "prefers metric units"});
    save.call(request(args, Some(operation), Some("op/1/step/0/call/a")))
        .await
        .expect("saved");
    seed(&store, "test:user:U9", "k2", "someone else's fact").await;

    let elsewhere = accepted_actor_operation(&store, "C7", "U7").await;
    let recall = RecallTool::new(store, Scope::User, 10);
    let result = recall
        .call(request(serde_json::json!({}), Some(elsewhere), Some("key")))
        .await
        .expect("recalled");
    assert!(result.text.contains("prefers metric units"));
    assert!(!result.text.contains("someone else's fact"));
}

#[tokio::test]
async fn user_scope_tools_fail_without_an_actor() {
    let store = MemoryStore::default();
    let operation = accepted_operation(&store, "C42").await;
    let tool = SaveTool::new(store, Scope::User);

    let error = tool
        .call(request(
            serde_json::json!({"content": "x"}),
            Some(operation),
            Some("key"),
        ))
        .await
        .expect_err("no actor");
    assert_eq!(error.code, "memory_actor_required");
}

#[tokio::test]
async fn user_scope_injection_skips_prompts_without_an_actor() {
    let store = MemoryStore::default();
    seed(&store, "test:user:U7", "k1", "known fact").await;
    let recorder = Recorder::default();
    let admission = MemoryAdmission::new(recorder.clone(), store, Scope::User, Some(10));

    admission
        .accept(&prompt("C42", "hello"))
        .await
        .expect("accepted");
    assert_eq!(recorder.accepted_text(), "hello");

    admission
        .accept(&prompt("C42", "hello").with_actor("U7"))
        .await
        .expect("accepted");
    assert_eq!(
        recorder.accepted_text(),
        "<memories>\n- known fact\n</memories>\n\nhello"
    );
}

#[tokio::test]
async fn injection_prepends_memories_oldest_first() {
    let store = MemoryStore::default();
    seed(&store, "test/C42", "k1", "first fact").await;
    seed(&store, "test/C42", "k2", "second fact").await;
    let recorder = Recorder::default();
    let admission = MemoryAdmission::new(recorder.clone(), store, Scope::Conversation, Some(10));

    admission
        .accept(&prompt("C42", "hello"))
        .await
        .expect("accepted");

    assert_eq!(
        recorder.accepted_text(),
        "<memories>\n- first fact\n- second fact\n</memories>\n\nhello"
    );
}

#[tokio::test]
async fn injection_caps_at_the_configured_limit_with_the_newest_kept() {
    let store = MemoryStore::default();
    seed(&store, "test/C42", "k1", "first fact").await;
    seed(&store, "test/C42", "k2", "second fact").await;
    let recorder = Recorder::default();
    let admission = MemoryAdmission::new(recorder.clone(), store, Scope::Conversation, Some(1));

    admission
        .accept(&prompt("C42", "hello"))
        .await
        .expect("accepted");

    assert_eq!(
        recorder.accepted_text(),
        "<memories>\n- second fact\n</memories>\n\nhello"
    );
}

#[tokio::test]
async fn disabled_injection_and_empty_scopes_leave_the_prompt_untouched() {
    let store = MemoryStore::default();
    seed(&store, "test/C42", "k1", "first fact").await;
    let recorder = Recorder::default();
    let disabled = MemoryAdmission::new(recorder.clone(), store.clone(), Scope::Conversation, None);
    disabled
        .accept(&prompt("C42", "hello"))
        .await
        .expect("accepted");
    assert_eq!(recorder.accepted_text(), "hello");

    let empty_scope = MemoryAdmission::new(recorder.clone(), store, Scope::Conversation, Some(10));
    empty_scope
        .accept(&prompt("C7", "hi"))
        .await
        .expect("accepted");
    assert_eq!(recorder.accepted_text(), "hi");
}

#[test]
fn the_default_config_disables_every_trigger() {
    let config = Config::default();
    assert!(!config.save);
    assert!(!config.recall_tool);
    assert_eq!(config.inject, None);
    assert_eq!(config.scope, Scope::Conversation);
}

#[tokio::test]
async fn enabled_config_yields_both_tools() {
    let config = Config {
        save: true,
        recall_tool: true,
        ..Config::default()
    };
    let tools = brainstone_memory::tools(&config, &MemoryStore::default());
    let names: Vec<&str> = tools.iter().map(|tool| tool.name()).collect();
    assert_eq!(names, vec!["MemorySave", "MemoryRecall"]);
    assert!(brainstone_memory::tools(&Config::default(), &MemoryStore::default()).is_empty());
}

#[test]
fn user_memory_cannot_be_injected_into_an_authenticated_shared_conversation() {
    let mut identity = agent::ExecutionIdentity {
        provider: "test".into(),
        tenant: "T1".into(),
        actor: "U1".into(),
        conversation: "C1".into(),
        private: false,
    };
    assert!(!Scope::User.permits(Some(&identity)));
    identity.private = true;
    assert!(Scope::User.permits(Some(&identity)));
    assert_eq!(
        brainstone_memory::Error::PrivateConversationRequired.code(),
        "memory_private_conversation_required"
    );
}
